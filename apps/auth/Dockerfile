FROM node:22-bookworm-slim
WORKDIR /app
COPY package.json ./
RUN npm install --omit=dev
COPY tsconfig.json ./
COPY src ./src
ENV PORT=3001
EXPOSE 3001
# Configuration comes from the environment in a container, so the .env file
# is optional rather than required.
CMD ["node", "--env-file-if-exists=.env", "--import", "tsx", "src/index.ts"]
