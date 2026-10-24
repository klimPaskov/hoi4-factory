# EXP-02: run the pinned upstream focus and source tests inside Linux.
# Build context: .work/hoi4-agent-tools at 25f5641bbc07c65a822ef576ac6e92bd12666880.
# Same pinned base image as the upstream Dockerfile. npm ci uses the shrinkwrap.
FROM node:22-bookworm-slim@sha256:53ada149d435c38b14476cb57e4a7da73c15595aba79bd6971b547ceb6d018bf
WORKDIR /app
COPY package.json npm-shrinkwrap.json ./
RUN npm ci
COPY --chown=node:node . .
USER node
ENTRYPOINT ["npx", "vitest", "run"]
