FROM node:22-alpine

WORKDIR /app

RUN corepack enable

COPY pnpm-lock.yaml pnpm-workspace.yaml ./
COPY frontend/package.json /app/frontend/package.json
RUN pnpm --dir frontend install --frozen-lockfile

COPY frontend /app/frontend
WORKDIR /app/frontend

CMD ["pnpm", "run", "dev"]
