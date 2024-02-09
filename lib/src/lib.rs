mod domain;
mod executor;
mod json_diff;
mod parser;

pub struct DocAssert<'a> {
    url: Option<&'a str>,
    doc_path: Option<&'a str>,
}

impl<'a> DocAssert<'a> {
    pub fn new() -> Self {
        Self {
            url: None,
            doc_path: None,
        }
    }

    pub fn with_url(mut self, url: &'a str) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_doc_path(mut self, doc_path: &'a str) -> Self {
        self.doc_path = Some(doc_path);
        self
    }

    pub async fn assert(mut self) -> Result<(), Vec<String>> {
        let url = self.url.take().expect("URL is required");
        let doc_path = self.doc_path.take().expect("Doc path is required");
        let test_cases = parser::parse(doc_path.to_string()).map_err(|e| vec![e])?;
        let mut errors = vec![];
        for tc in test_cases {
            if let Err(e) = executor::execute(url.clone(), tc).await {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<'a> Default for DocAssert<'a> {
    fn default() -> Self {
        Self::new()
    }
}
