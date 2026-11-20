/// Returns the OpenAPI 3.1.0 specification for BugHunter REST API v1.
pub fn openapi_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "BugHunter Universal Debugging API",
            "version": "1.0.0",
            "description": "Versioned REST API for BugHunter Core debugging workflows, providers, and reports."
        },
        "servers": [{ "url": "/api/v1" }],
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer"
                }
            },
            "schemas": {
                "InvestigationCreateRequest": {
                    "type": "object",
                    "required": ["repository"],
                    "properties": {
                        "repository": { "type": "string" },
                        "issue": { "type": "string" },
                        "files": { "type": "array", "items": { "type": "string" } },
                        "verification_required": { "type": "boolean" },
                        "provider_hint": { "type": "string" },
                        "model_hint": { "type": "string" }
                    }
                },
                "PatchVerifyRequest": {
                    "type": "object",
                    "required": ["repository"],
                    "properties": {
                        "repository": { "type": "string" },
                        "patch": { "type": "string" },
                        "issue": { "type": "string" }
                    }
                }
            }
        },
        "security": [{ "bearerAuth": [] }],
        "paths": {
            "/investigations": {
                "post": {
                    "summary": "Start an asynchronous debugging investigation",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/InvestigationCreateRequest" }
                            }
                        }
                    },
                    "responses": { "202": { "description": "Investigation accepted" } }
                }
            },
            "/investigations/{id}": {
                "get": {
                    "summary": "Get investigation status and result",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": { "200": { "description": "Investigation record" }, "404": { "description": "Not found" } }
                }
            },
            "/investigations/{id}/cancel": {
                "post": {
                    "summary": "Cancel a running investigation",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": { "200": { "description": "Investigation cancelled" } }
                }
            },
            "/investigations/{id}/report": {
                "get": {
                    "summary": "Get investigation report",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": { "200": { "description": "Structured report" } }
                }
            },
            "/repositories/analyze": {
                "post": {
                    "summary": "Analyze a repository",
                    "responses": { "200": { "description": "Repository analysis result" } }
                }
            },
            "/bugs/reproduce": {
                "post": {
                    "summary": "Reproduce a reported bug",
                    "responses": { "200": { "description": "Reproduction result" } }
                }
            },
            "/patches/generate": {
                "post": {
                    "summary": "Generate a candidate patch",
                    "responses": { "200": { "description": "Patch generation result" } }
                }
            },
            "/patches/verify": {
                "post": {
                    "summary": "Verify a candidate patch",
                    "responses": { "200": { "description": "Verification result" } }
                }
            },
            "/providers": {
                "get": {
                    "summary": "List configured model providers and capabilities",
                    "responses": { "200": { "description": "Provider capability list" } }
                }
            },
            "/health": {
                "get": {
                    "summary": "Service health check",
                    "security": [],
                    "responses": { "200": { "description": "Healthy" } }
                }
            }
        }
    })
}
