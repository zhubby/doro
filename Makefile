.PHONY: doro-ui control-plane agent

doro-ui:
	cd doro-ui && bun run dev

control-plane:
	cargo run -p doro-cli -- --log-level debug control-plane

agent:
	cargo run -p doro-cli -- --log-level debug agent
