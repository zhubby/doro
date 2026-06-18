.PHONY: dev doro-ui control-plane agent build-release install-doro
.PHONY: control-plane-service-install control-plane-service-enable-now control-plane-service-start control-plane-service-stop control-plane-service-restart control-plane-service-status control-plane-service-logs control-plane-service-uninstall
.PHONY: agent-service-install agent-service-enable-now agent-service-start agent-service-stop agent-service-restart agent-service-status agent-service-logs agent-service-uninstall
.PHONY: agent-config-file
.PHONY: control-plane-systemd-user control-plane-systemd-unit control-plane-systemd-install control-plane-systemd-enable-now control-plane-systemd-start control-plane-systemd-stop control-plane-systemd-restart control-plane-systemd-status control-plane-systemd-logs control-plane-systemd-uninstall
.PHONY: agent-systemd-user agent-systemd-config agent-systemd-unit agent-systemd-install agent-systemd-enable-now agent-systemd-start agent-systemd-stop agent-systemd-restart agent-systemd-status agent-systemd-logs agent-systemd-uninstall
.PHONY: control-plane-launchd-user control-plane-launchd-plist control-plane-launchd-install control-plane-launchd-enable-now control-plane-launchd-start control-plane-launchd-stop control-plane-launchd-restart control-plane-launchd-status control-plane-launchd-logs control-plane-launchd-uninstall
.PHONY: agent-launchd-user agent-launchd-config agent-launchd-plist agent-launchd-install agent-launchd-enable-now agent-launchd-start agent-launchd-stop agent-launchd-restart agent-launchd-status agent-launchd-logs agent-launchd-uninstall

DORO_UI_DEV = cd doro-ui && bun run dev
CONTROL_PLANE_DEV = cargo run -p doro-cli -- --log-level debug control-plane
AGENT_DEV = cargo run -p doro-cli -- --log-level debug agent

CARGO ?= cargo
INSTALL ?= install
SUDO ?= sudo
SYSTEMCTL ?= systemctl
LAUNCHCTL ?= launchctl
PLUTIL ?= plutil

UNAME_S := $(shell uname -s)
DORO_SERVICE_MANAGER ?= $(if $(filter Darwin,$(UNAME_S)),launchd,systemd)
DORO_SERVICE_USER_DEFAULT ?= $(if $(filter Darwin,$(UNAME_S)),$(shell id -un),doro)
DORO_SERVICE_GROUP_DEFAULT ?= $(if $(filter Darwin,$(UNAME_S)),$(shell id -gn),doro)
DORO_STATE_DIR_DEFAULT ?= $(if $(filter Darwin,$(UNAME_S)),/Library/Application Support/Doro,/var/lib/doro)

DORO_INSTALL_PREFIX ?= /usr/local
DORO_BIN_DIR ?= $(DORO_INSTALL_PREFIX)/bin
DORO_INSTALLED_BIN ?= $(DORO_BIN_DIR)/doro
DORO_RELEASE_BIN ?= target/release/doro

DORO_SYSTEMD_DIR ?= /etc/systemd/system
DORO_LAUNCHD_DOMAIN ?= system
DORO_LAUNCHD_DIR ?= /Library/LaunchDaemons

DORO_CONTROL_PLANE_SERVICE ?= doro-control-plane
DORO_CONTROL_PLANE_SERVICE_TEMPLATE ?= packaging/systemd/doro-control-plane.service.in
DORO_CONTROL_PLANE_SERVICE_FILE ?= $(DORO_SYSTEMD_DIR)/$(DORO_CONTROL_PLANE_SERVICE).service
DORO_CONTROL_PLANE_LAUNCHD_LABEL ?= com.doro.control-plane
DORO_CONTROL_PLANE_LAUNCHD_TEMPLATE ?= packaging/launchd/doro-control-plane.plist.in
DORO_CONTROL_PLANE_LAUNCHD_PLIST ?= $(DORO_LAUNCHD_DIR)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL).plist
DORO_CONTROL_PLANE_USER ?= $(DORO_SERVICE_USER_DEFAULT)
DORO_CONTROL_PLANE_GROUP ?= $(DORO_SERVICE_GROUP_DEFAULT)
DORO_CONTROL_PLANE_STATE_DIR ?= $(DORO_STATE_DIR_DEFAULT)
DORO_CONTROL_PLANE_LOG_DIR ?= $(DORO_CONTROL_PLANE_STATE_DIR)/logs
DORO_CONTROL_PLANE_CONSOLE_BIND ?= 0.0.0.0:8787
DORO_CONTROL_PLANE_AGENT_BIND ?= 0.0.0.0:8788
DORO_CONTROL_PLANE_STORE_BACKEND ?= postgres
DORO_CONTROL_PLANE_DATABASE_URL ?= postgres://doro:doro@127.0.0.1:5432/doro
DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS ?= 10
DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS ?= 1
DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS ?= 8
DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS ?= 300
DORO_CONTROL_PLANE_APPROVAL_POLICY ?= policy_and_human_approval
DORO_CONTROL_PLANE_REQUIRE_TLS ?= false
DORO_CONTROL_PLANE_AI_PROVIDER ?= disabled
DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV ?= OPENAI_API_KEY
DORO_CONTROL_PLANE_OPENAI_BASE_URL ?= https://api.openai.com/v1
DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL ?= gpt-4.1-mini
DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL ?= gpt-4.1-mini
DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS ?= 60
DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS ?= 12
DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS ?= 32
DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS ?= 30
DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS ?= 120
DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS ?= 86400
DORO_CONTROL_PLANE_RUST_LOG ?= doro_cli=info,doro_control_plane=info,tower_http=info

DORO_AGENT_SERVICE ?= doro-agent
DORO_AGENT_SERVICE_TEMPLATE ?= packaging/systemd/doro-agent.service.in
DORO_AGENT_SERVICE_FILE ?= $(DORO_SYSTEMD_DIR)/$(DORO_AGENT_SERVICE).service
DORO_AGENT_LAUNCHD_LABEL ?= com.doro.agent
DORO_AGENT_LAUNCHD_TEMPLATE ?= packaging/launchd/doro-agent.plist.in
DORO_AGENT_LAUNCHD_PLIST ?= $(DORO_LAUNCHD_DIR)/$(DORO_AGENT_LAUNCHD_LABEL).plist
DORO_AGENT_CONFIG ?= /etc/doro/agent.toml
DORO_AGENT_USER ?= $(DORO_SERVICE_USER_DEFAULT)
DORO_AGENT_GROUP ?= $(DORO_SERVICE_GROUP_DEFAULT)
DORO_AGENT_SUPPLEMENTARY_GROUPS ?=
DORO_AGENT_STATE_DIR ?= $(DORO_STATE_DIR_DEFAULT)
DORO_AGENT_LOG_DIR ?= $(DORO_AGENT_STATE_DIR)/logs
DORO_AGENT_CONTROL_PLANE_URL ?= http://127.0.0.1:8788
DORO_AGENT_HOSTNAME ?= $(shell hostname 2>/dev/null || echo doro-agent)
DORO_AGENT_RUST_LOG ?= doro_cli=info,doro_agent=info,tower_http=info

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

build-release:
	$(CARGO) build --release --locked -p doro-cli --bin doro

install-doro: build-release
	$(SUDO) $(INSTALL) -d -m 0755 "$(DORO_BIN_DIR)"
	$(SUDO) $(INSTALL) -m 0755 "$(DORO_RELEASE_BIN)" "$(DORO_INSTALLED_BIN)"

control-plane-service-install:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-install

control-plane-service-enable-now:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-enable-now

control-plane-service-start:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-start

control-plane-service-stop:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-stop

control-plane-service-restart:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-restart

control-plane-service-status:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-status

control-plane-service-logs:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-logs

control-plane-service-uninstall:
	$(MAKE) control-plane-$(DORO_SERVICE_MANAGER)-uninstall

agent-service-install:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-install

agent-service-enable-now:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-enable-now

agent-service-start:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-start

agent-service-stop:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-stop

agent-service-restart:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-restart

agent-service-status:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-status

agent-service-logs:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-logs

agent-service-uninstall:
	$(MAKE) agent-$(DORO_SERVICE_MANAGER)-uninstall

control-plane-systemd-user:
	@if ! getent group "$(DORO_CONTROL_PLANE_GROUP)" >/dev/null; then \
		$(SUDO) groupadd --system "$(DORO_CONTROL_PLANE_GROUP)"; \
	fi
	@if ! id -u "$(DORO_CONTROL_PLANE_USER)" >/dev/null 2>&1; then \
		$(SUDO) useradd --system --gid "$(DORO_CONTROL_PLANE_GROUP)" --home-dir "$(DORO_CONTROL_PLANE_STATE_DIR)" --shell /usr/sbin/nologin "$(DORO_CONTROL_PLANE_USER)"; \
	fi
	$(SUDO) $(INSTALL) -d -o "$(DORO_CONTROL_PLANE_USER)" -g "$(DORO_CONTROL_PLANE_GROUP)" -m 0750 "$(DORO_CONTROL_PLANE_STATE_DIR)"

control-plane-systemd-unit: install-doro control-plane-systemd-user
	@tmp=$$(mktemp); \
	sed \
		-e 's|@DORO_BIN@|$(DORO_INSTALLED_BIN)|g' \
		-e 's|@DORO_CONTROL_PLANE_USER@|$(DORO_CONTROL_PLANE_USER)|g' \
		-e 's|@DORO_CONTROL_PLANE_GROUP@|$(DORO_CONTROL_PLANE_GROUP)|g' \
		-e 's|@DORO_CONTROL_PLANE_STATE_DIR@|$(DORO_CONTROL_PLANE_STATE_DIR)|g' \
		-e 's|@DORO_CONTROL_PLANE_RUST_LOG@|$(DORO_CONTROL_PLANE_RUST_LOG)|g' \
		-e 's|@DORO_CONTROL_PLANE_CONSOLE_BIND@|$(DORO_CONTROL_PLANE_CONSOLE_BIND)|g' \
		-e 's|@DORO_CONTROL_PLANE_AGENT_BIND@|$(DORO_CONTROL_PLANE_AGENT_BIND)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_BACKEND@|$(DORO_CONTROL_PLANE_STORE_BACKEND)|g' \
		-e 's|@DORO_CONTROL_PLANE_DATABASE_URL@|$(DORO_CONTROL_PLANE_DATABASE_URL)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS@|$(DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS@|$(DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_APPROVAL_POLICY@|$(DORO_CONTROL_PLANE_APPROVAL_POLICY)|g' \
		-e 's|@DORO_CONTROL_PLANE_REQUIRE_TLS@|$(DORO_CONTROL_PLANE_REQUIRE_TLS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_PROVIDER@|$(DORO_CONTROL_PLANE_AI_PROVIDER)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV@|$(DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_BASE_URL@|$(DORO_CONTROL_PLANE_OPENAI_BASE_URL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL@|$(DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL@|$(DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS@|$(DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS@|$(DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS)|g' \
		"$(DORO_CONTROL_PLANE_SERVICE_TEMPLATE)" > "$$tmp"; \
	$(SUDO) $(INSTALL) -m 0644 "$$tmp" "$(DORO_CONTROL_PLANE_SERVICE_FILE)"; \
	rm -f "$$tmp"

control-plane-systemd-install: control-plane-systemd-unit
	$(SUDO) $(SYSTEMCTL) daemon-reload
	$(SUDO) $(SYSTEMCTL) enable "$(DORO_CONTROL_PLANE_SERVICE).service"
	@printf '\nInstalled %s as a systemd service.\n' "$(DORO_CONTROL_PLANE_SERVICE).service"
	@printf 'Configuration is supplied through service environment variables.\n'
	@printf 'Review database and security variables before first start, then run: make control-plane-systemd-start\n'

control-plane-systemd-enable-now: control-plane-systemd-install
	$(MAKE) control-plane-systemd-start

control-plane-systemd-start:
	$(SUDO) $(SYSTEMCTL) start "$(DORO_CONTROL_PLANE_SERVICE).service"

control-plane-systemd-stop:
	$(SUDO) $(SYSTEMCTL) stop "$(DORO_CONTROL_PLANE_SERVICE).service"

control-plane-systemd-restart:
	$(SUDO) $(SYSTEMCTL) restart "$(DORO_CONTROL_PLANE_SERVICE).service"

control-plane-systemd-status:
	-$(SYSTEMCTL) status "$(DORO_CONTROL_PLANE_SERVICE).service" || true

control-plane-systemd-logs:
	$(SUDO) journalctl -u "$(DORO_CONTROL_PLANE_SERVICE).service" -f

control-plane-systemd-uninstall:
	-$(SUDO) $(SYSTEMCTL) stop "$(DORO_CONTROL_PLANE_SERVICE).service"
	-$(SUDO) $(SYSTEMCTL) disable "$(DORO_CONTROL_PLANE_SERVICE).service"
	$(SUDO) rm -f "$(DORO_CONTROL_PLANE_SERVICE_FILE)"
	$(SUDO) $(SYSTEMCTL) daemon-reload

control-plane-launchd-user:
	@if ! id -u "$(DORO_CONTROL_PLANE_USER)" >/dev/null 2>&1; then \
		echo "launchd user '$(DORO_CONTROL_PLANE_USER)' does not exist"; \
		exit 1; \
	fi
	$(SUDO) $(INSTALL) -d -o "$(DORO_CONTROL_PLANE_USER)" -g "$(DORO_CONTROL_PLANE_GROUP)" -m 0750 "$(DORO_CONTROL_PLANE_STATE_DIR)"
	$(SUDO) $(INSTALL) -d -o "$(DORO_CONTROL_PLANE_USER)" -g "$(DORO_CONTROL_PLANE_GROUP)" -m 0750 "$(DORO_CONTROL_PLANE_LOG_DIR)"

control-plane-launchd-plist: install-doro control-plane-launchd-user
	@tmp=$$(mktemp); \
	sed \
		-e 's|@DORO_BIN@|$(DORO_INSTALLED_BIN)|g' \
		-e 's|@DORO_CONTROL_PLANE_USER@|$(DORO_CONTROL_PLANE_USER)|g' \
		-e 's|@DORO_CONTROL_PLANE_GROUP@|$(DORO_CONTROL_PLANE_GROUP)|g' \
		-e 's|@DORO_CONTROL_PLANE_STATE_DIR@|$(DORO_CONTROL_PLANE_STATE_DIR)|g' \
		-e 's|@DORO_CONTROL_PLANE_LOG_DIR@|$(DORO_CONTROL_PLANE_LOG_DIR)|g' \
		-e 's|@DORO_CONTROL_PLANE_RUST_LOG@|$(DORO_CONTROL_PLANE_RUST_LOG)|g' \
		-e 's|@DORO_CONTROL_PLANE_LAUNCHD_LABEL@|$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)|g' \
		-e 's|@DORO_CONTROL_PLANE_CONSOLE_BIND@|$(DORO_CONTROL_PLANE_CONSOLE_BIND)|g' \
		-e 's|@DORO_CONTROL_PLANE_AGENT_BIND@|$(DORO_CONTROL_PLANE_AGENT_BIND)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_BACKEND@|$(DORO_CONTROL_PLANE_STORE_BACKEND)|g' \
		-e 's|@DORO_CONTROL_PLANE_DATABASE_URL@|$(DORO_CONTROL_PLANE_DATABASE_URL)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS@|$(DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS@|$(DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_APPROVAL_POLICY@|$(DORO_CONTROL_PLANE_APPROVAL_POLICY)|g' \
		-e 's|@DORO_CONTROL_PLANE_REQUIRE_TLS@|$(DORO_CONTROL_PLANE_REQUIRE_TLS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_PROVIDER@|$(DORO_CONTROL_PLANE_AI_PROVIDER)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV@|$(DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_BASE_URL@|$(DORO_CONTROL_PLANE_OPENAI_BASE_URL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL@|$(DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL@|$(DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL)|g' \
		-e 's|@DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS@|$(DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS@|$(DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS)|g' \
		-e 's|@DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS@|$(DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS)|g' \
		"$(DORO_CONTROL_PLANE_LAUNCHD_TEMPLATE)" > "$$tmp"; \
	$(PLUTIL) -lint "$$tmp"; \
	$(SUDO) $(INSTALL) -d -m 0755 "$(DORO_LAUNCHD_DIR)"; \
	$(SUDO) $(INSTALL) -m 0644 "$$tmp" "$(DORO_CONTROL_PLANE_LAUNCHD_PLIST)"; \
	rm -f "$$tmp"

control-plane-launchd-install: control-plane-launchd-plist
	-$(SUDO) $(LAUNCHCTL) enable "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"
	@printf '\nInstalled %s as a launchd service.\n' "$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"
	@printf 'Plist: %s\n' "$(DORO_CONTROL_PLANE_LAUNCHD_PLIST)"
	@printf 'Configuration is supplied through service environment variables.\n'
	@printf 'Review database and security variables before first start, then run: make control-plane-launchd-start\n'

control-plane-launchd-enable-now: control-plane-launchd-install
	$(MAKE) control-plane-launchd-start

control-plane-launchd-start:
	$(SUDO) $(LAUNCHCTL) bootstrap "$(DORO_LAUNCHD_DOMAIN)" "$(DORO_CONTROL_PLANE_LAUNCHD_PLIST)" || $(SUDO) $(LAUNCHCTL) kickstart -k "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"

control-plane-launchd-stop:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"

control-plane-launchd-restart:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"
	$(SUDO) $(LAUNCHCTL) bootstrap "$(DORO_LAUNCHD_DOMAIN)" "$(DORO_CONTROL_PLANE_LAUNCHD_PLIST)"

control-plane-launchd-status:
	$(SUDO) $(LAUNCHCTL) print "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"

control-plane-launchd-logs:
	$(SUDO) tail -f "$(DORO_CONTROL_PLANE_LOG_DIR)/control-plane.out.log" "$(DORO_CONTROL_PLANE_LOG_DIR)/control-plane.err.log"

control-plane-launchd-uninstall:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_CONTROL_PLANE_LAUNCHD_LABEL)"
	$(SUDO) rm -f "$(DORO_CONTROL_PLANE_LAUNCHD_PLIST)"

agent-systemd-user:
	@if ! getent group "$(DORO_AGENT_GROUP)" >/dev/null; then \
		$(SUDO) groupadd --system "$(DORO_AGENT_GROUP)"; \
	fi
	@if ! id -u "$(DORO_AGENT_USER)" >/dev/null 2>&1; then \
		$(SUDO) useradd --system --gid "$(DORO_AGENT_GROUP)" --home-dir "$(DORO_AGENT_STATE_DIR)" --shell /usr/sbin/nologin "$(DORO_AGENT_USER)"; \
	fi
	$(SUDO) $(INSTALL) -d -o "$(DORO_AGENT_USER)" -g "$(DORO_AGENT_GROUP)" -m 0750 "$(DORO_AGENT_STATE_DIR)"
	@config_dir=$$(dirname "$(DORO_AGENT_CONFIG)"); \
	if [ "$$config_dir" != "." ]; then \
		$(SUDO) $(INSTALL) -d -o root -g "$(DORO_AGENT_GROUP)" -m 0750 "$$config_dir"; \
	fi
	@if [ -n "$(DORO_AGENT_SUPPLEMENTARY_GROUPS)" ]; then \
		for group in $(DORO_AGENT_SUPPLEMENTARY_GROUPS); do \
			if getent group "$$group" >/dev/null; then \
				$(SUDO) usermod -a -G "$$group" "$(DORO_AGENT_USER)"; \
			else \
				echo "warning: supplementary group '$$group' does not exist; skipping"; \
			fi; \
		done; \
	fi

agent-systemd-config: agent-systemd-user
	$(MAKE) agent-config-file

agent-config-file:
	@if [ ! -f "$(DORO_AGENT_CONFIG)" ]; then \
		tmp=$$(mktemp); \
		{ \
			printf '%s\n' '[agent]'; \
			printf 'control_plane_url = "%s"\n' "$(DORO_AGENT_CONTROL_PLANE_URL)"; \
			printf 'hostname = "%s"\n' "$(DORO_AGENT_HOSTNAME)"; \
			printf 'heartbeat_interval_seconds = 30\n'; \
			printf 'metrics_interval_seconds = 10\n'; \
			printf '\n%s\n' '[reliability]'; \
			printf 'event_spool_enabled = true\n'; \
			printf 'event_spool_path = "%s/.doro/agent-event-spool"\n' "$(DORO_AGENT_STATE_DIR)"; \
			printf 'event_spool_max_files = 256\n'; \
			printf 'event_spool_max_bytes = 67108864\n'; \
			printf 'command_cancel_grace_seconds = 5\n'; \
			printf 'preflight_enabled = true\n'; \
			printf '\n%s\n' '[websites]'; \
			printf 'http_bind = "127.0.0.1:8080"\n'; \
			printf '\n%s\n' '[ai]'; \
			printf 'provider = "disabled"\n'; \
		} > "$$tmp"; \
		$(SUDO) $(INSTALL) -o "$(DORO_AGENT_USER)" -g "$(DORO_AGENT_GROUP)" -m 0600 "$$tmp" "$(DORO_AGENT_CONFIG)"; \
		rm -f "$$tmp"; \
	else \
		$(SUDO) chown "$(DORO_AGENT_USER):$(DORO_AGENT_GROUP)" "$(DORO_AGENT_CONFIG)"; \
		$(SUDO) chmod 0600 "$(DORO_AGENT_CONFIG)"; \
	fi

agent-systemd-unit: install-doro agent-systemd-config
	@tmp=$$(mktemp); \
	sed \
		-e 's|@DORO_BIN@|$(DORO_INSTALLED_BIN)|g' \
		-e 's|@DORO_AGENT_CONFIG@|$(DORO_AGENT_CONFIG)|g' \
		-e 's|@DORO_AGENT_USER@|$(DORO_AGENT_USER)|g' \
		-e 's|@DORO_AGENT_GROUP@|$(DORO_AGENT_GROUP)|g' \
		-e 's|@DORO_AGENT_SUPPLEMENTARY_GROUPS@|$(DORO_AGENT_SUPPLEMENTARY_GROUPS)|g' \
		-e 's|@DORO_AGENT_STATE_DIR@|$(DORO_AGENT_STATE_DIR)|g' \
		-e 's|@DORO_AGENT_RUST_LOG@|$(DORO_AGENT_RUST_LOG)|g' \
		"$(DORO_AGENT_SERVICE_TEMPLATE)" > "$$tmp"; \
	$(SUDO) $(INSTALL) -m 0644 "$$tmp" "$(DORO_AGENT_SERVICE_FILE)"; \
	rm -f "$$tmp"

agent-systemd-install: agent-systemd-unit
	$(SUDO) $(SYSTEMCTL) daemon-reload
	$(SUDO) $(SYSTEMCTL) enable "$(DORO_AGENT_SERVICE).service"
	@printf '\nInstalled %s as a systemd service.\n' "$(DORO_AGENT_SERVICE).service"
	@printf 'Config: %s\n' "$(DORO_AGENT_CONFIG)"
	@printf 'Set enrollment_token in the config before first start, then run: make agent-systemd-start\n'

agent-systemd-enable-now: agent-systemd-install
	$(MAKE) agent-systemd-start

agent-systemd-start:
	$(SUDO) $(SYSTEMCTL) start "$(DORO_AGENT_SERVICE).service"

agent-systemd-stop:
	$(SUDO) $(SYSTEMCTL) stop "$(DORO_AGENT_SERVICE).service"

agent-systemd-restart:
	$(SUDO) $(SYSTEMCTL) restart "$(DORO_AGENT_SERVICE).service"

agent-systemd-status:
	-$(SYSTEMCTL) status "$(DORO_AGENT_SERVICE).service" || true

agent-systemd-logs:
	$(SUDO) journalctl -u "$(DORO_AGENT_SERVICE).service" -f

agent-systemd-uninstall:
	-$(SUDO) $(SYSTEMCTL) stop "$(DORO_AGENT_SERVICE).service"
	-$(SUDO) $(SYSTEMCTL) disable "$(DORO_AGENT_SERVICE).service"
	$(SUDO) rm -f "$(DORO_AGENT_SERVICE_FILE)"
	$(SUDO) $(SYSTEMCTL) daemon-reload

agent-launchd-user:
	@if ! id -u "$(DORO_AGENT_USER)" >/dev/null 2>&1; then \
		echo "launchd user '$(DORO_AGENT_USER)' does not exist"; \
		exit 1; \
	fi
	$(SUDO) $(INSTALL) -d -o "$(DORO_AGENT_USER)" -g "$(DORO_AGENT_GROUP)" -m 0750 "$(DORO_AGENT_STATE_DIR)"
	$(SUDO) $(INSTALL) -d -o "$(DORO_AGENT_USER)" -g "$(DORO_AGENT_GROUP)" -m 0750 "$(DORO_AGENT_LOG_DIR)"
	@config_dir=$$(dirname "$(DORO_AGENT_CONFIG)"); \
	if [ "$$config_dir" != "." ]; then \
		$(SUDO) $(INSTALL) -d -o root -g "$(DORO_AGENT_GROUP)" -m 0750 "$$config_dir"; \
	fi

agent-launchd-config: agent-launchd-user
	$(MAKE) agent-config-file

agent-launchd-plist: install-doro agent-launchd-config
	@tmp=$$(mktemp); \
	sed \
		-e 's|@DORO_BIN@|$(DORO_INSTALLED_BIN)|g' \
		-e 's|@DORO_AGENT_CONFIG@|$(DORO_AGENT_CONFIG)|g' \
		-e 's|@DORO_AGENT_USER@|$(DORO_AGENT_USER)|g' \
		-e 's|@DORO_AGENT_GROUP@|$(DORO_AGENT_GROUP)|g' \
		-e 's|@DORO_AGENT_STATE_DIR@|$(DORO_AGENT_STATE_DIR)|g' \
		-e 's|@DORO_AGENT_LOG_DIR@|$(DORO_AGENT_LOG_DIR)|g' \
		-e 's|@DORO_AGENT_RUST_LOG@|$(DORO_AGENT_RUST_LOG)|g' \
		-e 's|@DORO_AGENT_LAUNCHD_LABEL@|$(DORO_AGENT_LAUNCHD_LABEL)|g' \
		"$(DORO_AGENT_LAUNCHD_TEMPLATE)" > "$$tmp"; \
	$(PLUTIL) -lint "$$tmp"; \
	$(SUDO) $(INSTALL) -d -m 0755 "$(DORO_LAUNCHD_DIR)"; \
	$(SUDO) $(INSTALL) -m 0644 "$$tmp" "$(DORO_AGENT_LAUNCHD_PLIST)"; \
	rm -f "$$tmp"

agent-launchd-install: agent-launchd-plist
	-$(SUDO) $(LAUNCHCTL) enable "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"
	@printf '\nInstalled %s as a launchd service.\n' "$(DORO_AGENT_LAUNCHD_LABEL)"
	@printf 'Plist: %s\n' "$(DORO_AGENT_LAUNCHD_PLIST)"
	@printf 'Config: %s\n' "$(DORO_AGENT_CONFIG)"
	@printf 'Set enrollment_token in the config before first start, then run: make agent-launchd-start\n'

agent-launchd-enable-now: agent-launchd-install
	$(MAKE) agent-launchd-start

agent-launchd-start:
	$(SUDO) $(LAUNCHCTL) bootstrap "$(DORO_LAUNCHD_DOMAIN)" "$(DORO_AGENT_LAUNCHD_PLIST)" || $(SUDO) $(LAUNCHCTL) kickstart -k "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"

agent-launchd-stop:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"

agent-launchd-restart:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"
	$(SUDO) $(LAUNCHCTL) bootstrap "$(DORO_LAUNCHD_DOMAIN)" "$(DORO_AGENT_LAUNCHD_PLIST)"

agent-launchd-status:
	$(SUDO) $(LAUNCHCTL) print "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"

agent-launchd-logs:
	$(SUDO) tail -f "$(DORO_AGENT_LOG_DIR)/agent.out.log" "$(DORO_AGENT_LOG_DIR)/agent.err.log"

agent-launchd-uninstall:
	-$(SUDO) $(LAUNCHCTL) bootout "$(DORO_LAUNCHD_DOMAIN)/$(DORO_AGENT_LAUNCHD_LABEL)"
	$(SUDO) rm -f "$(DORO_AGENT_LAUNCHD_PLIST)"
