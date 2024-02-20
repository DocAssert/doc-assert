# Variables
API_URL = http://localhost:8000
DOC_ASSERT_BIN = ./../../target/debug/doc-assert

# Targets
tests:
	@echo "Running tests against backend..."
	@for file in ./readmes/*; do\
		echo "\nRunning test for $$file\n";\
		$(DOC_ASSERT_BIN) --url http://127.0.0.1:8000 $$file;\
	done

.PHONY: tests

# Default target
all: tests

