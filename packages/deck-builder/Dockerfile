# Build from the repository root:
# docker build -f packages/deck-builder/Dockerfile -t deck-builder .
# To bump: inspect node:26-slim's manifest, replace both digests, rebuild and
# run the gate + image dependency checks before staging/promoting the change.
FROM node:26-slim@sha256:36793bcbb160a6a16cede1e62b93f84e26b78a972a8a4c0b879c1c12f22761ec AS dependencies
WORKDIR /app
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages/deck-builder/package.json packages/deck-builder/package.json
RUN npm ci --workspace=packages/deck-builder --include-workspace-root

FROM dependencies AS build
COPY packages/deck-builder/src packages/deck-builder/src
COPY packages/deck-builder/web packages/deck-builder/web
COPY packages/deck-builder/tsconfig.json packages/deck-builder/vite.config.ts packages/deck-builder/
COPY packages/deck-builder/scripts/check-runtime.mjs packages/deck-builder/scripts/check-runtime.mjs
RUN npm exec --workspace=packages/deck-builder -- vite build \
    && node packages/deck-builder/scripts/check-runtime.mjs --manifest

FROM dependencies AS production-dependencies
RUN npm prune --omit=dev --workspace=packages/deck-builder --include-workspace-root \
    && mkdir -p packages/deck-builder/node_modules

FROM node:26-slim@sha256:36793bcbb160a6a16cede1e62b93f84e26b78a972a8a4c0b879c1c12f22761ec AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends util-linux \
    && rm -rf /var/lib/apt/lists/*
ENV NODE_ENV=production DECKBUILDER_DATA=/data DECKBUILDER_PORT=8787
WORKDIR /app
COPY --from=production-dependencies /app/node_modules ./node_modules
COPY --from=production-dependencies /app/packages/deck-builder/node_modules ./packages/deck-builder/node_modules
COPY --from=production-dependencies /app/package.json ./package.json
COPY --from=build /app/packages/deck-builder/package.json ./packages/deck-builder/package.json
COPY --from=build /app/packages/deck-builder/src ./packages/deck-builder/src
COPY --from=build /app/packages/deck-builder/web/dist ./packages/deck-builder/web/dist
COPY --from=build /app/packages/deck-builder/runtime-imports.json ./packages/deck-builder/runtime-imports.json
COPY packages/deck-builder/scripts/container-entrypoint.sh /usr/local/bin/deck-builder-entrypoint
COPY packages/deck-builder/scripts/check-runtime.mjs ./packages/deck-builder/scripts/check-runtime.mjs
RUN node packages/deck-builder/scripts/check-runtime.mjs
# Railway mounts volumes as root. Its entrypoint briefly prepares /data and
# drops privileges; Node, refresh and backup always run as UID 1000.
RUN mkdir /data && chown node:node /data
USER node
WORKDIR /app/packages/deck-builder
EXPOSE 8787
ENTRYPOINT ["sh", "/usr/local/bin/deck-builder-entrypoint"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+process.env.DECKBUILDER_PORT+'/api/health').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "src/server.ts"]
