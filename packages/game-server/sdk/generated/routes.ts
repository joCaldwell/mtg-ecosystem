/* Generated from Rust protocol::ROUTES; do not edit. */
export const ROUTES = {
  "get_client_config": {
    "method": "GET",
    "path": "/client-config.json",
    "response_schema": "client_config"
  },
  "get_presentation": {
    "method": "GET",
    "path": "/v1/matches/{match_id}/presentation",
    "response_schema": "presentation_descriptor"
  },
  "get_resume": {
    "method": "GET",
    "path": "/v1/matches/{match_id}/resume",
    "response_schema": "resume"
  },
  "get_manifest": {
    "method": "GET",
    "path": "/v1/matches/{match_id}/manifest",
    "response_schema": "manifest"
  },
  "submit_command": {
    "method": "POST",
    "path": "/v1/matches/{match_id}/commands",
    "response_schema": "submission"
  },
  "poll_events": {
    "method": "GET",
    "path": "/v1/matches/{match_id}/events",
    "response_schema": "stream"
  },
  "command_execution": {
    "method": "GET",
    "path": "/v1/matches/{match_id}/commands/{command_id}",
    "response_schema": "execution"
  },
  "cancel_command": {
    "method": "POST",
    "path": "/v1/matches/{match_id}/commands/{command_id}/cancel",
    "response_schema": "submission"
  }
} as const;
