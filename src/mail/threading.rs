use rayon::prelude::*;
use std::collections::HashMap;

use super::types::Envelope;

/// Build a flat, display-ready list with threading metadata.
/// Messages are grouped into threads, sorted by most recent message (descending),
/// and within each thread, sorted chronologically (ascending).
/// Linear chains are collapsed (depth 1), branching creates new levels (max depth 3).
/// Uses parallel processing for performance.
pub fn build_threaded_list(envelopes: Vec<Envelope>) -> Vec<Envelope> {
    if envelopes.is_empty() {
        return envelopes;
    }

    let len = envelopes.len();

    // 1. Build message_id -> index map (parallel)
    let id_to_idx: HashMap<String, usize> = envelopes
        .par_iter()
        .enumerate()
        .filter_map(|(i, env)| env.message_id.as_ref().map(|mid| (mid.clone(), i)))
        .collect();

    // 2. Build parent relationships in parallel (avoiding self-references and cycles)
    let parent: Vec<Option<usize>> = envelopes
        .par_iter()
        .enumerate()
        .map(|(i, env)| {
            // First try in_reply_to
            if let Some(ref reply_to) = env.in_reply_to {
                if let Some(&parent_idx) = id_to_idx.get(reply_to) {
                    if parent_idx != i {
                        return Some(parent_idx);
                    }
                }
            }
            // Fall back to last entry in references
            for ref_id in env.references.iter().rev() {
                if let Some(&parent_idx) = id_to_idx.get(ref_id) {
                    if parent_idx != i {
                        return Some(parent_idx);
                    }
                }
            }
            None
        })
        .collect();

    // 3. Build children map using parallel fold + reduce
    let children: HashMap<usize, Vec<usize>> = parent
        .par_iter()
        .enumerate()
        .filter_map(|(i, p)| p.map(|parent_idx| (parent_idx, i)))
        .fold(
            HashMap::new,
            |mut map: HashMap<usize, Vec<usize>>, (parent_idx, child_idx)| {
                map.entry(parent_idx).or_default().push(child_idx);
                map
            },
        )
        .reduce(HashMap::new, |mut a, b| {
            for (k, mut v) in b {
                a.entry(k).or_default().append(&mut v);
            }
            a
        });

    // Sort children by date (parallel over parents)
    let children: HashMap<usize, Vec<usize>> = children
        .into_par_iter()
        .map(|(parent_idx, mut kids)| {
            kids.sort_by(|&a, &b| {
                let date_a = envelopes[a].date.as_deref().unwrap_or("");
                let date_b = envelopes[b].date.as_deref().unwrap_or("");
                date_a.cmp(date_b)
            });
            (parent_idx, kids)
        })
        .collect();

    // 4. Find thread roots - sequential with path compression (union-find style)
    let mut thread_root: Vec<usize> = (0..len).collect();
    for i in 0..len {
        if parent[i].is_some() {
            // Find root with cycle protection
            let mut current = i;
            let mut steps = 0;
            while let Some(p) = parent[current] {
                current = p;
                steps += 1;
                if steps > 1000 {
                    break; // Cycle detected, stop
                }
            }
            let root = current;

            // Path compression
            current = i;
            while let Some(p) = parent[current] {
                thread_root[current] = root;
                current = p;
            }
            thread_root[i] = root;
        }
    }

    // 5. Collect unique roots and group by thread (parallel fold + reduce)
    let threads: HashMap<usize, Vec<usize>> = thread_root
        .par_iter()
        .enumerate()
        .fold(
            HashMap::new,
            |mut map: HashMap<usize, Vec<usize>>, (i, &root)| {
                map.entry(root).or_default().push(i);
                map
            },
        )
        .reduce(HashMap::new, |mut a, b| {
            for (k, mut v) in b {
                a.entry(k).or_default().append(&mut v);
            }
            a
        });

    // 6. For each thread, find the most recent message date (parallel)
    let thread_last_date: HashMap<usize, String> = threads
        .par_iter()
        .map(|(&root, indices)| {
            let max_date = indices
                .iter()
                .filter_map(|&i| envelopes[i].date.as_ref())
                .max()
                .cloned()
                .unwrap_or_default();
            (root, max_date)
        })
        .collect();

    // 7. Get sorted roots
    let mut roots: Vec<usize> = threads.keys().copied().collect();
    roots.par_sort_by(|&a, &b| {
        let date_a = thread_last_date.get(&a).map(|s| s.as_str()).unwrap_or("");
        let date_b = thread_last_date.get(&b).map(|s| s.as_str()).unwrap_or("");
        date_b.cmp(date_a) // Descending
    });

    // 8. Process each thread in parallel and collect full Envelope results
    let children_ref = &children;
    let envelopes_ref = &envelopes;

    let thread_results: Vec<Vec<Envelope>> = roots
        .par_iter()
        .map(|&root_idx| {
            // Collect messages in this thread using DFS
            let mut thread_messages: Vec<(usize, usize, bool)> = Vec::new();
            collect_thread_dfs(
                root_idx,
                0,
                true,
                children_ref,
                envelopes_ref,
                &mut thread_messages,
            );

            // Compute display depths
            let display_depths = compute_display_depths(&thread_messages, children_ref);

            // Build result envelopes directly
            let thread_len = thread_messages.len();
            thread_messages
                .into_iter()
                .enumerate()
                .map(|(i, (msg_idx, _raw_depth, is_last_sibling))| {
                    let display_depth = display_depths[i];
                    let is_last = i == thread_len - 1;
                    let prefix = compute_tree_prefix(display_depth, is_last_sibling);

                    let mut env = envelopes_ref[msg_idx].clone();
                    env.thread_depth = display_depth;
                    env.display_depth = display_depth;
                    env.is_last_in_thread = is_last;
                    env.tree_prefix = prefix;
                    env
                })
                .collect()
        })
        .collect();

    // 9. Flatten results
    thread_results.into_iter().flatten().collect()
}

/// DFS traversal to collect messages in a thread
fn collect_thread_dfs(
    idx: usize,
    depth: usize,
    is_last: bool,
    children: &HashMap<usize, Vec<usize>>,
    envelopes: &[Envelope],
    result: &mut Vec<(usize, usize, bool)>,
) {
    result.push((idx, depth, is_last));

    if let Some(kids) = children.get(&idx) {
        let kids_len = kids.len();
        for (i, &child_idx) in kids.iter().enumerate() {
            let child_is_last = i == kids_len - 1;
            collect_thread_dfs(
                child_idx,
                depth + 1,
                child_is_last,
                children,
                envelopes,
                result,
            );
        }
    }
}

/// Compute display depths with linear chain collapsing.
/// A linear chain (single child at each level) stays at depth 1.
/// Branching (multiple children) increases depth.
/// Depth is capped at 3.
fn compute_display_depths(
    messages: &[(usize, usize, bool)],
    children: &HashMap<usize, Vec<usize>>,
) -> Vec<usize> {
    if messages.is_empty() {
        return vec![];
    }

    let mut display_depths = vec![0; messages.len()];

    // First message is always depth 0 (root)
    display_depths[0] = 0;

    for pos in 1..messages.len() {
        let (_msg_idx, raw_depth, _) = messages[pos];

        if raw_depth == 0 {
            display_depths[pos] = 0;
            continue;
        }

        // Find the parent position (most recent message with depth < raw_depth)
        let mut parent_pos = pos - 1;
        while parent_pos > 0 && messages[parent_pos].1 >= raw_depth {
            parent_pos -= 1;
        }

        let parent_idx = messages[parent_pos].0;
        let parent_display_depth = display_depths[parent_pos];

        // Check if parent has multiple children (branching)
        let num_children = children.get(&parent_idx).map(|c| c.len()).unwrap_or(0);

        if num_children > 1 {
            // Branching - increment depth
            display_depths[pos] = (parent_display_depth + 1).min(3);
        } else {
            // Linear chain - stay at same depth (but at least 1 if we have a parent)
            display_depths[pos] = if parent_display_depth == 0 {
                1
            } else {
                parent_display_depth
            };
        }
    }

    display_depths
}

/// Generate tree prefix string based on depth and position
fn compute_tree_prefix(depth: usize, is_last_sibling: bool) -> String {
    if depth == 0 {
        return String::new();
    }

    if depth > 3 {
        return format!("[{}] ", depth);
    }

    let mut prefix = String::new();

    for _ in 0..depth.saturating_sub(1) {
        prefix.push_str("│  ");
    }

    if is_last_sibling {
        prefix.push_str("└─ ");
    } else {
        prefix.push_str("├─ ");
    }

    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_prefix() {
        assert_eq!(compute_tree_prefix(0, true), "");
        assert_eq!(compute_tree_prefix(1, false), "├─ ");
        assert_eq!(compute_tree_prefix(1, true), "└─ ");
        assert_eq!(compute_tree_prefix(2, false), "│  ├─ ");
        assert_eq!(compute_tree_prefix(2, true), "│  └─ ");
        assert_eq!(compute_tree_prefix(3, true), "│  │  └─ ");
        assert_eq!(compute_tree_prefix(4, true), "[4] ");
    }
}
