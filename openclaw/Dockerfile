FROM ghcr.io/openclaw/openclaw:latest

# Fix matrix extension: symlink root app to node_modules/openclaw
# The compiled matrix extension imports from 'openclaw/plugin-sdk/...'
# Node.js tries to resolve from node_modules/openclaw but openclaw IS the root package
RUN mkdir -p /app/dist/extensions/matrix/node_modules && \
    ln -sf /app /app/dist/extensions/matrix/node_modules/openclaw

# Install uv, create venv, install dependencies, and add to PATH
RUN curl -LsSf https://astral.sh/uv/install.sh | sh && \
    export PATH="$HOME/.local/bin:$PATH" && \
    uv venv /home/node/.openclaw/venv && \
    VIRTUAL_ENV=/home/node/.openclaw/venv uv pip install requests Pillow && \
    echo 'export PATH="/home/node/.openclaw/venv/bin:$PATH"' >> /home/node/.bashrc && \
    echo 'export PATH="/home/node/.openclaw/venv/bin:$PATH"' >> /home/node/.profile

ENV PATH="/home/node/.openclaw/venv/bin:/home/node/.local/bin:$PATH"

# Copy complete workspace (skills, agent config, docs)
COPY --chown=node:node workspace-build/ /home/node/.openclaw/workspace/

ADD --chown=node:node ./openclaw.json /home/node/.openclaw/openclaw.json
