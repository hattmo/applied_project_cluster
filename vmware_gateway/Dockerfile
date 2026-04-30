FROM python:3.14-slim-bookworm
COPY --from=docker.io/astral/uv:latest /uv /uvx /bin/
COPY ./README.md ./main.py ./pyproject.toml ./uv.lock /opt/
WORKDIR /opt
RUN uv sync
ENTRYPOINT ["uv", "run", "gunicorn", "main:app", "-b", "0.0.0.0:8080"]
