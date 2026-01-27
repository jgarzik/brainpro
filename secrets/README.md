# Docker Secrets

This directory holds API key files for Docker secrets. Each file should contain the raw key value (no quotes, no newline at end).

## Required Files

Create these files with your API keys:

```bash
echo -n "your-venice-api-key" > venice_api_key.txt
echo -n "your-openai-api-key" > openai_api_key.txt
echo -n "your-anthropic-api-key" > anthropic_api_key.txt
echo -n "$(openssl rand -hex 32)" > brainpro_gateway_token.txt
```

## Security Notes

- This directory is gitignored - never commit API keys
- Files are mounted read-only at `/run/secrets/` in the container
- The entrypoint exports them as environment variables
