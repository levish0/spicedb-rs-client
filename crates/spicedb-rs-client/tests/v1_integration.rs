use std::{
    env,
    net::TcpListener,
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serial_test::serial;
use spicedb_rs_client::{
    Client, ClientBuilder,
    v1::{
        CheckPermissionRequest, Consistency, LookupResourcesRequest, ObjectReference,
        ReadSchemaRequest, Relationship, RelationshipUpdate, SubjectReference,
        WriteRelationshipsRequest, WriteSchemaRequest, check_permission_response, consistency,
        relationship_update,
    },
};
use tokio::time::sleep;
use tonic::Code;

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after UNIX_EPOCH")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

fn fully_consistent() -> Consistency {
    Consistency {
        requirement: Some(consistency::Requirement::FullyConsistent(true)),
    }
}

struct SpiceDbServer {
    endpoint: String,
    container_id: Option<String>,
}

impl SpiceDbServer {
    fn start() -> Self {
        if let Ok(endpoint) = env::var("SPICEDB_ENDPOINT") {
            return Self {
                endpoint,
                container_id: None,
            };
        }

        let host_port = find_available_port();
        let port_mapping = format!("127.0.0.1:{host_port}:50051");
        let output = Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--rm")
            .arg("-p")
            .arg(&port_mapping)
            .arg("authzed/spicedb:latest")
            .arg("serve-testing")
            .output()
            .expect("failed to execute docker run");

        if !output.status.success() {
            panic!(
                "failed to start SpiceDB container:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            panic!("docker run returned an empty container id");
        }

        Self {
            endpoint: format!("127.0.0.1:{host_port}"),
            container_id: Some(container_id),
        }
    }
}

impl Drop for SpiceDbServer {
    fn drop(&mut self) {
        if let Some(container_id) = &self.container_id {
            let _ = Command::new("docker")
                .arg("rm")
                .arg("-f")
                .arg(container_id)
                .output();
        }
    }
}

fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind temporary port")
        .local_addr()
        .expect("failed to get temporary port")
        .port()
}

async fn test_client(server: &SpiceDbServer, test_name: &str) -> Client {
    let token = unique_id(test_name);

    for _ in 0..40 {
        if let Ok(client) = ClientBuilder::new(server.endpoint.clone())
            .insecure(true)
            .with_token(token.clone())
            .connect()
            .await
        {
            match client.schema().read_schema(ReadSchemaRequest {}).await {
                Ok(_) => return client,
                Err(status)
                    if !matches!(
                        status.code(),
                        Code::Unavailable | Code::Unknown | Code::DeadlineExceeded
                    ) =>
                {
                    return client;
                }
                Err(_) => {}
            }
        }
        sleep(Duration::from_millis(250)).await;
    }

    panic!(
        "failed to connect to SpiceDB at {}. Ensure Docker is available, or set SPICEDB_ENDPOINT to an existing SpiceDB server",
        server.endpoint
    );
}

async fn write_test_schema(client: &mut Client) {
    let schema = r#"
definition test/user {}

definition test/document {
    relation viewer: test/user
    permission view = viewer
}
"#;

    client
        .schema()
        .write_schema(WriteSchemaRequest {
            schema: schema.to_string(),
        })
        .await
        .expect("schema write should succeed");
}

#[tokio::test]
#[serial]
async fn schema_roundtrip_works() {
    let server = SpiceDbServer::start();
    let mut client = test_client(&server, "schema-roundtrip").await;
    write_test_schema(&mut client).await;

    let response = client
        .schema()
        .read_schema(ReadSchemaRequest {})
        .await
        .expect("schema read should succeed")
        .into_inner();

    assert!(response.schema_text.contains("definition test/document"));
    assert!(response.schema_text.contains("permission view = viewer"));
}

#[tokio::test]
#[serial]
async fn check_permission_matches_written_relationships() {
    let server = SpiceDbServer::start();
    let mut client = test_client(&server, "check-permission").await;
    write_test_schema(&mut client).await;

    let resource = ObjectReference {
        object_type: "test/document".to_string(),
        object_id: unique_id("doc"),
    };
    let subject_object = ObjectReference {
        object_type: "test/user".to_string(),
        object_id: unique_id("user"),
    };
    let subject = SubjectReference {
        object: Some(subject_object.clone()),
        optional_relation: String::new(),
    };

    client
        .permissions()
        .write_relationships(WriteRelationshipsRequest {
            updates: vec![RelationshipUpdate {
                operation: relationship_update::Operation::Create as i32,
                relationship: Some(Relationship {
                    resource: Some(resource.clone()),
                    relation: "viewer".to_string(),
                    subject: Some(subject.clone()),
                    optional_caveat: None,
                    optional_expires_at: None,
                }),
            }],
            ..Default::default()
        })
        .await
        .expect("relationship write should succeed");

    let response = client
        .permissions()
        .check_permission(CheckPermissionRequest {
            consistency: Some(fully_consistent()),
            resource: Some(resource),
            permission: "view".to_string(),
            subject: Some(subject),
            ..Default::default()
        })
        .await
        .expect("check permission should succeed")
        .into_inner();

    assert_eq!(
        response.permissionship,
        check_permission_response::Permissionship::HasPermission as i32
    );
}

#[tokio::test]
#[serial]
async fn lookup_resources_stream_returns_expected_items() {
    let server = SpiceDbServer::start();
    let mut client = test_client(&server, "lookup-resources").await;
    write_test_schema(&mut client).await;

    let user_id = unique_id("user");
    let doc_id = unique_id("doc");

    let user = ObjectReference {
        object_type: "test/user".to_string(),
        object_id: user_id,
    };
    let doc = ObjectReference {
        object_type: "test/document".to_string(),
        object_id: doc_id.clone(),
    };

    client
        .permissions()
        .write_relationships(WriteRelationshipsRequest {
            updates: vec![RelationshipUpdate {
                operation: relationship_update::Operation::Create as i32,
                relationship: Some(Relationship {
                    resource: Some(doc),
                    relation: "viewer".to_string(),
                    subject: Some(SubjectReference {
                        object: Some(user.clone()),
                        optional_relation: String::new(),
                    }),
                    optional_caveat: None,
                    optional_expires_at: None,
                }),
            }],
            ..Default::default()
        })
        .await
        .expect("relationship write should succeed");

    let mut stream = client
        .permissions()
        .lookup_resources(LookupResourcesRequest {
            consistency: Some(fully_consistent()),
            resource_object_type: "test/document".to_string(),
            permission: "view".to_string(),
            subject: Some(SubjectReference {
                object: Some(user),
                optional_relation: String::new(),
            }),
            ..Default::default()
        })
        .await
        .expect("lookup resources should succeed")
        .into_inner();

    let mut returned_ids = Vec::new();
    while let Some(item) = stream
        .message()
        .await
        .expect("stream read should not error")
    {
        returned_ids.push(item.resource_object_id);
    }

    assert!(returned_ids.contains(&doc_id));
}
