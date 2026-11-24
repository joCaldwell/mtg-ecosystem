export declare const ROUTES: {
    readonly get_client_config: {
        readonly method: "GET";
        readonly path: "/client-config.json";
        readonly response_schema: "client_config";
    };
    readonly get_presentation: {
        readonly method: "GET";
        readonly path: "/v1/matches/{match_id}/presentation";
        readonly response_schema: "presentation_descriptor";
    };
    readonly get_resume: {
        readonly method: "GET";
        readonly path: "/v1/matches/{match_id}/resume";
        readonly response_schema: "resume";
    };
    readonly get_manifest: {
        readonly method: "GET";
        readonly path: "/v1/matches/{match_id}/manifest";
        readonly response_schema: "manifest";
    };
    readonly submit_command: {
        readonly method: "POST";
        readonly path: "/v1/matches/{match_id}/commands";
        readonly response_schema: "submission";
    };
    readonly poll_events: {
        readonly method: "GET";
        readonly path: "/v1/matches/{match_id}/events";
        readonly response_schema: "stream";
    };
    readonly command_execution: {
        readonly method: "GET";
        readonly path: "/v1/matches/{match_id}/commands/{command_id}";
        readonly response_schema: "execution";
    };
    readonly cancel_command: {
        readonly method: "POST";
        readonly path: "/v1/matches/{match_id}/commands/{command_id}/cancel";
        readonly response_schema: "submission";
    };
};
