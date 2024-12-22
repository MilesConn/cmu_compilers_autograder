use anyhow::Result;

struct Pipeline<'a, R>
where
    R: Send + Sync,
{
    prereqs: Box<dyn FnOnce() -> Result<()> + 'a>,
    parallel_steps: Box<dyn Fn() -> Result<Vec<R>> + Send + Sync + 'a>,
    next_pipeline: Option<Box<Pipeline<'a, R>>>,
}

impl<'a, R> Pipeline<'a, R>
where
    R: Send + Sync,
{
    pub fn new(
        prereqs: impl FnOnce() -> Result<()> + 'a,
        parallel_steps: impl Fn() -> Result<Vec<R>> + Send + Sync + 'a,
    ) -> Self {
        Self {
            prereqs: Box::new(prereqs),
            parallel_steps: Box::new(parallel_steps),
            next_pipeline: None,
        }
    }

    pub fn then(mut self, next: Pipeline<'a, R>) -> Self {
        self.next_pipeline = Some(Box::new(next));
        self
    }

    pub fn execute(self) -> Result<Vec<R>> {
        // Run prerequisites
        (self.prereqs)()?;

        // Execute parallel steps
        let results = (self.parallel_steps)()?;

        // Execute next pipeline if it exists
        if let Some(next) = self.next_pipeline {
            next.execute()?;
        }

        Ok(results)
    }
}
