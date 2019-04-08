//! # [`depmap`](crate) - Dependency map manipulation
//!
//! Dependency maps are useful for any application that works with stuff that depends on other
//! stuff.
//!
//! Cyclic dependencies are found and handled.

/// An error type.
pub enum Error<T, E> {
    /// A cyclic dependency error.
    CyclicDep(Vec<T>),
    /// A user-defined error.
    UserDef(E),
}

impl<T, E> From<E> for Error<T, E> {
    fn from(err: E) -> Self {
        Error::UserDef(err)
    }
}

/// The dependency map.
pub struct DepMap<T: PartialEq> {
    /// A list of lists of things that need to be worked on at the same level.
    /// The first of each list is 'active'; the others will be handled in reverse order.
    /// The last few lists might be empty, called free lists.
    list: Vec<Vec<T>>,
    /// The result list.
    result: Vec<T>,
    /// The number of used lists.
    used: usize,
}

impl<T: PartialEq> DepMap<T> {
    /// Creates a new [`DepMap`] from an initial list.
    pub fn new(list: Vec<T>) -> Self {
        Self {
            used: if list.is_empty() {0} else {1},
            list: vec![list],
            result: Vec::new(),
        }
    }

    /// Runs through a whole dependency map using a single producer function.
    ///
    /// This is probably what one should use.
    pub fn process<F, I, E>(initial: Vec<T>, mut f: F) -> Result<Vec<T>, Error<T, E>>
    where F: FnMut(&T) -> Result<I, E>, I: Iterator<Item = T> {
        // The current map.
        let mut state = Self::new(initial);
        loop {
            match state.destroy() {
                Ok(res) => break Ok(res),
                Err(map) => state = map,
            };

            // Not empty; Process
            state.add(&mut f)?
                .map(|deps| deps.len())
                .map_or(Ok(()), |len| Err(state.list.iter_mut()
                    .take(state.used)
                    .skip(state.used - len)
                    .map(|list| list.swap_remove(0))
                    .collect::<Vec<_>>()))
                .map_err(Error::CyclicDep)?;
        }
    }

    /// Whether the map is empty (i.e nothing needs to be worked on).
    pub fn is_empty(&self) -> bool {
        self.used == 0
    }

    /// Returns the result list if the dependency map is empty.
    ///
    /// If it is not empty, then an error is returned with the whole map.
    pub fn destroy(self) -> Result<Vec<T>, Self> {
        if self.is_empty() {
            Ok(self.result)
        } else {
            Err(self)
        }
    }

    /// Adds the latest target's dependencies at the end, removing those already done and
    /// returning cyclic dependency errors (if any).
    ///
    /// When cyclic dependency errors occur, the target is retained but its dependencies are not.
    /// Skips everything if the depmap is empty.
    pub fn add<F, I, E>(&mut self, f: F) -> Result<Option<Vec<&T>>, E>
    where F: FnOnce(&T) -> Result<I, E>, I: Iterator<Item = T> {
        if self.is_empty() {
            return Ok(None);
        }

        // Get a free list.
        let mut free = self.get_free();
        // Add to it the new targets.
        for tgt in (f)(&self.list[self.used - 1][0])? {
            if self.result.iter().any(|done| done == &tgt) {
                // Found in result list; already done, skip
                continue;
            } else if let Some(pos) = self.list[0..self.used].iter()
                    .map(|list| &list[0]).position(|cur| cur == &tgt) {
                // Found in active target list; cyclic dependency, fail
                free.clear();
                self.list.push(free);
                return Ok(Some(self.list[pos..self.used].iter().map(|list| &list[0]).collect()))
            } else {
                // No issues; unhandled, add to list
                free.push(tgt)
            }
        }
        // If the list is empty, then the target is a node; drop active targets.
        // Otherwise, add the list to the used space.
        if free.is_empty() {
            self.drop_cur();
        } else {
            // Add the free length to the used space.
            let len = self.list.len();
            self.list.push(free);
            self.list.swap(len, self.used);
            self.used += 1;
        }
        Ok(None)
    }

    /// Returns a free list.
    fn get_free(&mut self) -> Vec<T> {
        if self.used < self.list.len() {
            // Some free lengths exist; Pop one off.
            self.list.pop().unwrap()
        } else {
            // No free lengths exist; Just make a new list.
            Vec::new()
        }
    }

    /// Drops as many active targets as possible, beginning from the end.
    fn drop_cur(&mut self) {
        // While used lengths exist:
        while self.used > 0 {
            // Get the latest used list.
            let list = &mut self.list[self.used - 1];
            // Drop the active target into the result list.
            self.result.push(list.swap_remove(0));
            // While the list isn't empty, search for a target that has not been handled yet.
            let found = loop {
                if list.is_empty() {
                    break false
                }

                let tgt = &list[0];

                // In result list: Already handled, remove and continue
                // Otherwise: found unhandled, stop
                if self.result.iter().any(|done| done == tgt) {
                    list.swap_remove(0);
                } else {
                    break true
                }
            };
            // If found: Stop.
            // Otherwise: Mark as free (now empty) and move on.
            if found {
                break
            } else {
                self.used -= 1;
            }
        }
    }
}
