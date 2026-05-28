# Doro UI

Frontend workspace for Doro, built with Bun, Next.js, Tailwind CSS, and shadcn/ui.

```bash
bun install
bun run dev
```

The UI reads the control-plane API from `NEXT_PUBLIC_DORO_CONTROL_PLANE_URL`, falling back to `http://127.0.0.1:8787`.

The New Host command infers the Agent endpoint as `http://<browser-hostname>:8788`. Set `NEXT_PUBLIC_DORO_AGENT_CONTROL_PLANE_URL` when the Agent endpoint uses a different public URL.

The shadcn/ui configuration lives in `components.json`; shared UI primitives live in `components/ui`.
