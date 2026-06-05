.PHONY: dev doro-ui control-plane agent

DORO_UI_DEV = cd doro-ui && bun run dev
CONTROL_PLANE_DEV = cargo run -p doro-cli -- --log-level debug control-plane
AGENT_DEV = cargo run -p doro-cli -- --log-level debug agent

dev:
	@bash -c '\
		pids=(); \
		start() { \
			setsid bash -c "$$1" & \
			pids+=($$!); \
		}; \
		stop_all() { \
			trap - INT TERM EXIT; \
			for pid in "$${pids[@]}"; do \
				kill -TERM "-$$pid" 2>/dev/null || true; \
			done; \
			wait "$${pids[@]}" 2>/dev/null || true; \
		}; \
		on_signal() { \
			stop_all; \
			exit 130; \
		}; \
		trap on_signal INT TERM; \
		trap stop_all EXIT; \
		start "$(CONTROL_PLANE_DEV)"; \
		start "$(AGENT_DEV)"; \
		start "$(DORO_UI_DEV)"; \
		wait -n "$${pids[@]}"; \
		status=$$?; \
		stop_all; \
		exit $$status; \
	'

doro-ui:
	$(DORO_UI_DEV)

control-plane:
	$(CONTROL_PLANE_DEV)

agent:
	$(AGENT_DEV)
