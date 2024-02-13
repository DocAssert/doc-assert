.PHONY: build run

blog-add:
	curl -s http://127.0.0.1:8000/blog -X POST -H "Content-Type: application/json" -d '{"title": "My First Blog", "body": "This is my sample blog"}'

blog-update:
	curl -s http://127.0.0.1:8000/blog/$(blog_id) -X PUT -H "Content-Type: application/json" -d '{"title": "Updated First Blog", "body": "This is update sample blog"}'

blog-get:
	curl -s http://127.0.0.1:8000/blog/$(blog_id)

blog-delete:
	curl -s http://127.0.0.1:8000/blog/$(blog_id) -X DELETE -H "Content-Type: application/json"

blog-list:
	curl -s http://127.0.0.1:8000/blog -X GET -H "Content-Type: application/json"

censor:
	curl -s http://127.0.0.1:8000/admin/censor -X POST -H "Content-Type: application/json" -d '{"pattern": "Blog", "replacement": "Page"}'