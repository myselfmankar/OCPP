.PHONY: test unit-test e2e-test steve-e2e-down

test:
	cargo test
	./scripts/steve-e2e.sh

unit-test:
	cargo test

e2e-test:
	./scripts/steve-e2e.sh

steve-e2e-down:
	docker compose -p $${STEVE_E2E_PROJECT:-ocpp_steve_e2e} -f target/steve-e2e/docker-compose.yml down -v

