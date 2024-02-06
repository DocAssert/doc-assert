mod domain;
mod executor;
mod json_diff;
mod parser;

pub struct DocAssert {
    url: Option<String>,
    doc_path: Option<String>,
}

impl DocAssert {
    pub fn new() -> Self {
        Self {
            url: None,
            doc_path: None,
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_doc_path(mut self, doc_path: String) -> Self {
        self.doc_path = Some(doc_path);
        self
    }

    pub async fn assert(mut self) -> Result<(), Vec<String>> {
        let url = self.url.take().expect("URL is required");
        let doc_path = self.doc_path.take().expect("Doc path is required");
        let test_cases = parser::parse(doc_path).map_err(|e| vec![e])?;
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

impl Default for DocAssert {
    fn default() -> Self {
        Self::new()
    }
}
