FROM node:22-bookworm-slim

WORKDIR /workspace

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/operator-bff/package.json apps/operator-bff/package.json
RUN pnpm install --frozen-lockfile --filter operator-bff...

COPY apps/operator-bff apps/operator-bff
RUN pnpm --filter operator-bff build

ENV NODE_ENV=production
USER node
WORKDIR /workspace/apps/operator-bff
EXPOSE 8090

CMD ["node", "dist/main.js"]
