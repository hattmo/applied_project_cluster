# NPC UI

Web interface for NPC (Non-Player Character) autonomous VM operator.

## Stack

- **Backend:** Rust (Axum web framework)
- **Frontend:** React (Vite build tool)
- **Container:** Multi-stage Docker build

## Project Structure

```
npc_ui/
├── backend/          # Rust Axum backend
│   ├── Cargo.toml
│   └── src/
├── frontend/         # React frontend
│   ├── package.json
│   └── src/
├── Dockerfile
└── README.md
```

## Development

### Backend

```bash
cd backend
cargo run
```

### Frontend

```bash
cd frontend
npm install
npm run dev
```

### Docker

```bash
docker build -t npc-ui .
docker run -p 3000:3000 npc-ui
```

## API

The backend provides a REST API for interacting with the NPC autonomous VM operator.

## License

MIT
