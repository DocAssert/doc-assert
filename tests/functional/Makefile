# Variables
API_URL = http://localhost:8000
DOC_ASSERT_BIN = ./../../target/debug/doc-assert
BACKEND_BIN = sample-api
BACKEND_DIR =  ../sample-api

# Targets
tests:
	@echo "Running tests against backend..."
	@for file in ./readmes/*; do\
		echo "\nRunning test for $$file\n";\
		$(DOC_ASSERT_BIN) --url http://127.0.0.1:8000 $$file;\
	done

sanity:
	@echo "Running sanity tests against backend..."
	$(DOC_ASSERT_BIN) --url http://127.0.0.1:8000 ../../README.md

backend-stop:
	@echo "Stopping backend..."
	@pkill -f $(BACKEND_BIN) || true
	@rm -f $(BACKEND_DIR)/backend.log
	
backend-start:
	@echo "Starting backend..."
	@touch ./backend.log
	@cd $(BACKEND_DIR) && cargo run &> ./backend.log &

backend-wait:
	@echo "Waiting for backend to start..."
	@while ! nc -z localhost 8000; do sleep 1; done

.PHONY: tests sanity backend-stop backend-start backend-wait

# Default target
all: tests

