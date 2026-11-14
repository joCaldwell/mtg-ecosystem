# Build from the repository root. The existing deployment Dockerfile path stays
# stable; Multiverse owns the executable product and deck-builder is its backend.
# docker build -f packages/deck-builder/Dockerfile -t multiverse .
FROM node:26-slim@sha256:36793bcbb160a6a16cede1e62b93f84e26b78a972a8a4c0b879c1c12f22761ec AS dependencies
WORKDIR /app
COPY package.json package-lock.json tsconfig.base.json ./
COPY packages/deck-builder/package.json packages/deck-builder/package.json
COPY packages/multiverse/package.json packages/multiverse/package.json
COPY packages/game-ui/package.json packages/game-ui/package.json
COPY packages/game-client/package.json packages/game-client/package.json
COPY packages/game-server/sdk/package.json packages/game-server/sdk/package.json
RUN npm ci --workspace=packages/multiverse --include-workspace-root

FROM dependencies AS build
COPY packages/deck-builder/src packages/deck-builder/src
COPY packages/multiverse/src packages/multiverse/src
COPY packages/multiverse/web packages/multiverse/web
COPY packages/multiverse/tsconfig.json packages/multiverse/vite.config.ts packages/multiverse/
COPY packages/game-ui/src packages/game-ui/src
COPY packages/game-client/src/preview packages/game-client/src/preview
COPY packages/game-client/src/features/decisions packages/game-client/src/features/decisions
COPY packages/game-client/src/decisions packages/game-client/src/decisions
COPY packages/game-client/src/presentation/opening-fixture.ts packages/game-client/src/presentation/opening-fixture.ts
COPY packages/game-client/fixtures/foundations-opening packages/game-client/fixtures/foundations-opening
COPY packages/game-client/scripts/verify-preview.mjs packages/game-client/scripts/verify-preview.mjs
COPY packages/deck-builder/scripts/check-runtime.mjs packages/deck-builder/scripts/check-runtime.mjs
RUN npm exec --workspace=packages/multiverse -- vite build \
    && node packages/deck-builder/scripts/check-runtime.mjs --manifest \
    && node packages/deck-builder/scripts/check-runtime.mjs --manifest --package packages/multiverse

FROM dependencies AS production-dependencies
RUN npm prune --omit=dev --workspace=packages/multiverse --include-workspace-root \
    && mkdir -p packages/deck-builder/node_modules packages/multiverse/node_modules

FROM node:26-slim@sha256:36793bcbb160a6a16cede1e62b93f84e26b78a972a8a4c0b879c1c12f22761ec AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends util-linux \
    && rm -rf /var/lib/apt/lists/*
ENV NODE_ENV=production DECKBUILDER_DATA=/data DECKBUILDER_PORT=8787
WORKDIR /app
COPY --from=production-dependencies /app/node_modules ./node_modules
COPY --from=production-dependencies /app/packages/deck-builder/node_modules ./packages/deck-builder/node_modules
COPY --from=production-dependencies /app/packages/multiverse/node_modules ./packages/multiverse/node_modules
COPY --from=production-dependencies /app/package.json ./package.json
COPY --from=build /app/packages/deck-builder/package.json ./packages/deck-builder/package.json
COPY --from=build /app/packages/deck-builder/src ./packages/deck-builder/src
COPY --from=build /app/packages/deck-builder/runtime-imports.json ./packages/deck-builder/runtime-imports.json
COPY --from=build /app/packages/multiverse/package.json ./packages/multiverse/package.json
COPY --from=build /app/packages/multiverse/src ./packages/multiverse/src
COPY --from=build /app/packages/multiverse/web/dist ./packages/multiverse/web/dist
COPY --from=build /app/packages/multiverse/runtime-imports.json ./packages/multiverse/runtime-imports.json
COPY packages/deck-builder/scripts/container-entrypoint.sh /usr/local/bin/deck-builder-entrypoint
COPY packages/deck-builder/scripts/check-runtime.mjs ./packages/deck-builder/scripts/check-runtime.mjs
RUN node packages/deck-builder/scripts/check-runtime.mjs \
    && node packages/deck-builder/scripts/check-runtime.mjs --package packages/multiverse
# Preserve the existing mounted data path and UID behavior. No database migration
# or production promotion is performed by this source ownership refactor.
RUN mkdir /data && chown node:node /data
USER node
WORKDIR /app/packages/deck-builder
EXPOSE 8787
ENTRYPOINT ["sh", "/usr/local/bin/deck-builder-entrypoint"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+process.env.DECKBUILDER_PORT+'/api/health').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "../multiverse/src/server.ts"]
