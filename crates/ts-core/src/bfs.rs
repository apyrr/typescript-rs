use std::hash::Hash;
use std::sync::Arc;

use ts_collections::{MapEntry, OrderedMap, SyncSet};

use crate::identity;

pub struct BreadthFirstSearchResult<N> {
    pub stopped: bool,
    pub path: Vec<N>,
}

struct BreadthFirstSearchJob<N> {
    node: N,
    parent: Option<Arc<BreadthFirstSearchJob<N>>>,
}

pub struct BreadthFirstSearchLevel<K, N> {
    jobs: OrderedMap<K, Arc<BreadthFirstSearchJob<N>>>,
}

impl<K: Eq + Hash + Clone, N: Clone> BreadthFirstSearchLevel<K, N> {
    pub fn has(&self, key: &K) -> bool {
        self.jobs.has(key)
    }

    pub fn delete(&mut self, key: &K) {
        self.jobs.delete(key);
    }

    pub fn range(&self, mut f: impl FnMut(N) -> bool) {
        for job in self.jobs.values() {
            if !f(job.node.clone()) {
                return;
            }
        }
    }
}

pub struct BreadthFirstSearchOptions<K, N> {
    // Visited is a set of nodes that have already been visited.
    // If nil, a new set will be created.
    pub visited: Option<SyncSet<K>>,
    // PreprocessLevel is a function that, if provided, will be called
    // before each level, giving the caller an opportunity to remove nodes.
    pub preprocess_level: Option<BreadthFirstSearchPreprocessLevel<K, N>>,
}

pub type BreadthFirstSearchPreprocessLevel<K, N> =
    Box<dyn FnMut(&mut BreadthFirstSearchLevel<K, N>)>;

impl<K, N> Default for BreadthFirstSearchOptions<K, N> {
    fn default() -> Self {
        Self {
            visited: None,
            preprocess_level: None,
        }
    }
}

// BreadthFirstSearchParallel performs a breadth-first search on a graph
// starting from the given node. It processes nodes in parallel and returns the path
// from the first node that satisfies the `visit` function back to the start node.
pub fn breadth_first_search_parallel<N>(
    start: N,
    neighbors: impl FnMut(N) -> Vec<N>,
    visit: impl FnMut(N) -> (bool, bool),
) -> BreadthFirstSearchResult<N>
where
    N: Eq + Hash + Clone,
{
    breadth_first_search_parallel_ex(
        start,
        neighbors,
        visit,
        BreadthFirstSearchOptions::default(),
        identity,
    )
}

// BreadthFirstSearchParallelEx is an extension of BreadthFirstSearchParallel that allows
// the caller to pass a pre-seeded set of already-visited nodes and a preprocessing function
// that can be used to remove nodes from each level before parallel processing.
pub fn breadth_first_search_parallel_ex<K, N>(
    start: N,
    mut neighbors: impl FnMut(N) -> Vec<N>,
    mut visit: impl FnMut(N) -> (bool, bool),
    mut options: BreadthFirstSearchOptions<K, N>,
    mut get_key: impl FnMut(N) -> K,
) -> BreadthFirstSearchResult<N>
where
    K: Eq + Hash + Clone,
    N: Clone,
{
    let visited = options.visited.unwrap_or_default();

    let mut fallback: Option<Arc<BreadthFirstSearchJob<N>>> = None;
    let create_path = |mut job: Option<Arc<BreadthFirstSearchJob<N>>>| -> Vec<N> {
        let mut path = Vec::new();
        while let Some(current) = job {
            path.push(current.node.clone());
            job = current.parent.clone();
        }
        path
    };

    let mut level = OrderedMap::from_list(vec![MapEntry {
        key: get_key(start.clone()),
        value: Arc::new(BreadthFirstSearchJob {
            node: start,
            parent: None,
        }),
    }]);
    while level.size() > 0 {
        if let Some(preprocess_level) = options.preprocess_level.as_mut() {
            let mut current_level = BreadthFirstSearchLevel { jobs: level };
            preprocess_level(&mut current_level);
            level = current_level.jobs;
        }

        let mut next_jobs = OrderedMap::default();
        for j in level.values() {
            let j = j.clone();
            if !visited.add_if_absent(get_key(j.node.clone())) {
                continue;
            }

            let (is_result, stop) = visit(j.node.clone());
            if is_result {
                if stop {
                    return BreadthFirstSearchResult {
                        stopped: true,
                        path: create_path(Some(j)),
                    };
                }
                if fallback.is_none() {
                    fallback = Some(j.clone());
                }
            }

            for child in neighbors(j.node.clone()) {
                let child = Arc::new(BreadthFirstSearchJob {
                    node: child,
                    parent: Some(j.clone()),
                });
                let key = get_key(child.node.clone());
                if !next_jobs.has(&key) {
                    next_jobs.set(key, child);
                }
            }
        }

        level = next_jobs;
    }
    BreadthFirstSearchResult {
        stopped: false,
        path: create_path(fallback.clone()),
    }
}
