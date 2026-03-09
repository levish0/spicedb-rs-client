use std::{
    collections::{BTreeMap, HashSet},
    env,
    net::TcpListener,
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use prost_types::{Struct, Value, value};
use serial_test::serial;
use spicedb_rs_client::{
    Client, ClientBuilder,
    v1::{
        CheckBulkPermissionsRequest, CheckBulkPermissionsRequestItem, CheckPermissionRequest,
        CheckPermissionResponse, Consistency, ContextualizedCaveat, ExportBulkRelationshipsRequest,
        ImportBulkRelationshipsRequest, LookupResourcesRequest, LookupSubjectsRequest,
        ObjectReference, ReadRelationshipsRequest, ReadSchemaRequest, Relationship,
        RelationshipFilter, RelationshipUpdate, SubjectReference, WriteRelationshipsRequest,
        WriteSchemaRequest, check_bulk_permissions_pair, check_permission_response, consistency,
        relationship_update,
    },
};
use tokio::time::sleep;
use tokio_stream::iter;
use tonic::Code;

const TEST_SCHEMA: &str = r#"
caveat likes_harry_potter(likes bool) {
    likes == true
}

definition test/post {
    relation writer: test/user
    relation reader: test/user
    relation caveated_reader: test/user with likes_harry_potter

    permission write = writer
    permission view = reader + writer
    permission view_as_fan = caveated_reader + writer
}

definition test/user {}
"#;

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

fn bool_struct(key: &str, value: bool) -> Struct {
    let mut fields = BTreeMap::new();
    fields.insert(
        key.to_string(),
        Value {
            kind: Some(value::Kind::BoolValue(value)),
        },
    );
    Struct { fields }
}

fn string_struct(pairs: &[(&str, &str)]) -> Struct {
    let mut fields = BTreeMap::new();
    for (key, value) in pairs {
        fields.insert(
            (*key).to_string(),
            Value {
                kind: Some(value::Kind::StringValue((*value).to_string())),
            },
        );
    }
    Struct { fields }
}

fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind temporary port")
        .local_addr()
        .expect("failed to get temporary port")
        .port()
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

async fn test_client(server: &SpiceDbServer, test_name: &str) -> Client {
    test_client_with_token(server, &unique_id(test_name)).await
}

async fn test_client_with_token(server: &SpiceDbServer, token: &str) -> Client {
    for _ in 0..60 {
        if let Ok(client) = ClientBuilder::new(server.endpoint.clone())
            .insecure(true)
            .with_token(token.to_string())
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

async fn write_test_schema(client: &Client) {
    client
        .schema()
        .write_schema(WriteSchemaRequest {
            schema: TEST_SCHEMA.to_string(),
        })
        .await
        .expect("schema write should succeed");
}

#[derive(Clone)]
struct TestTuples {
    emilia: SubjectReference,
    beatrice: SubjectReference,
    post_one: ObjectReference,
    post_two: ObjectReference,
}

async fn write_test_tuples(client: &Client) -> TestTuples {
    let emilia_object = ObjectReference {
        object_type: "test/user".to_string(),
        object_id: unique_id("emilia"),
    };
    let beatrice_object = ObjectReference {
        object_type: "test/user".to_string(),
        object_id: unique_id("beatrice"),
    };
    let post_one = ObjectReference {
        object_type: "test/post".to_string(),
        object_id: unique_id("post-one"),
    };
    let post_two = ObjectReference {
        object_type: "test/post".to_string(),
        object_id: unique_id("post-two"),
    };

    let emilia = SubjectReference {
        object: Some(emilia_object.clone()),
        optional_relation: String::new(),
    };
    let beatrice = SubjectReference {
        object: Some(beatrice_object.clone()),
        optional_relation: String::new(),
    };

    client
        .permissions()
        .write_relationships(WriteRelationshipsRequest {
            updates: vec![
                RelationshipUpdate {
                    operation: relationship_update::Operation::Create as i32,
                    relationship: Some(Relationship {
                        resource: Some(post_one.clone()),
                        relation: "writer".to_string(),
                        subject: Some(emilia.clone()),
                        optional_caveat: None,
                        optional_expires_at: None,
                    }),
                },
                RelationshipUpdate {
                    operation: relationship_update::Operation::Create as i32,
                    relationship: Some(Relationship {
                        resource: Some(post_two.clone()),
                        relation: "writer".to_string(),
                        subject: Some(emilia.clone()),
                        optional_caveat: None,
                        optional_expires_at: None,
                    }),
                },
                RelationshipUpdate {
                    operation: relationship_update::Operation::Create as i32,
                    relationship: Some(Relationship {
                        resource: Some(post_one.clone()),
                        relation: "reader".to_string(),
                        subject: Some(beatrice.clone()),
                        optional_caveat: None,
                        optional_expires_at: None,
                    }),
                },
                RelationshipUpdate {
                    operation: relationship_update::Operation::Create as i32,
                    relationship: Some(Relationship {
                        resource: Some(post_one.clone()),
                        relation: "caveated_reader".to_string(),
                        subject: Some(beatrice.clone()),
                        optional_caveat: Some(ContextualizedCaveat {
                            caveat_name: "likes_harry_potter".to_string(),
                            context: None,
                        }),
                        optional_expires_at: None,
                    }),
                },
            ],
            ..Default::default()
        })
        .await
        .expect("relationship write should succeed");

    TestTuples {
        emilia,
        beatrice,
        post_one,
        post_two,
    }
}

async fn check_permissionship(
    client: &Client,
    resource: ObjectReference,
    permission: &str,
    subject: SubjectReference,
    context: Option<Struct>,
) -> CheckPermissionResponse {
    client
        .permissions()
        .check_permission(CheckPermissionRequest {
            consistency: Some(fully_consistent()),
            resource: Some(resource),
            permission: permission.to_string(),
            subject: Some(subject),
            context,
            ..Default::default()
        })
        .await
        .expect("check permission should succeed")
        .into_inner()
}

#[tokio::test]
#[serial]
async fn schema_roundtrip_works() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "schema-roundtrip").await;
    write_test_schema(&client).await;

    let response = client
        .schema()
        .read_schema(ReadSchemaRequest {})
        .await
        .expect("schema read should succeed")
        .into_inner();

    assert!(response.schema_text.contains("definition test/post"));
    assert!(
        response
            .schema_text
            .contains("permission view_as_fan = caveated_reader + writer")
    );
}

#[tokio::test]
#[serial]
async fn check_unknown_namespace_returns_failed_precondition() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "unknown-namespace").await;

    let status = client
        .permissions()
        .check_permission(CheckPermissionRequest {
            resource: Some(ObjectReference {
                object_type: "test/missing".to_string(),
                object_id: "abc".to_string(),
            }),
            permission: "view".to_string(),
            subject: Some(SubjectReference {
                object: Some(ObjectReference {
                    object_type: "test/user".to_string(),
                    object_id: "user-1".to_string(),
                }),
                optional_relation: String::new(),
            }),
            ..Default::default()
        })
        .await
        .expect_err("unknown namespace check should fail");

    assert_eq!(status.code(), Code::FailedPrecondition);
}

#[tokio::test]
#[serial]
async fn check_permission_matrix_matches_relationships() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "check-matrix").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let emilia_view = check_permissionship(
        &client,
        tuples.post_one.clone(),
        "view",
        tuples.emilia.clone(),
        None,
    )
    .await;
    assert_eq!(
        emilia_view.permissionship,
        check_permission_response::Permissionship::HasPermission as i32
    );

    let emilia_write = check_permissionship(
        &client,
        tuples.post_one.clone(),
        "write",
        tuples.emilia.clone(),
        None,
    )
    .await;
    assert_eq!(
        emilia_write.permissionship,
        check_permission_response::Permissionship::HasPermission as i32
    );

    let beatrice_view = check_permissionship(
        &client,
        tuples.post_one.clone(),
        "view",
        tuples.beatrice.clone(),
        None,
    )
    .await;
    assert_eq!(
        beatrice_view.permissionship,
        check_permission_response::Permissionship::HasPermission as i32
    );

    let beatrice_write =
        check_permissionship(&client, tuples.post_one, "write", tuples.beatrice, None).await;
    assert_eq!(
        beatrice_write.permissionship,
        check_permission_response::Permissionship::NoPermission as i32
    );
}

#[tokio::test]
#[serial]
async fn caveated_check_behaves_like_official_clients() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "caveated-check").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let has_permission = check_permissionship(
        &client,
        tuples.post_one.clone(),
        "view_as_fan",
        tuples.beatrice.clone(),
        Some(bool_struct("likes", true)),
    )
    .await;
    assert_eq!(
        has_permission.permissionship,
        check_permission_response::Permissionship::HasPermission as i32
    );

    let no_permission = check_permissionship(
        &client,
        tuples.post_one.clone(),
        "view_as_fan",
        tuples.beatrice.clone(),
        Some(bool_struct("likes", false)),
    )
    .await;
    assert_eq!(
        no_permission.permissionship,
        check_permission_response::Permissionship::NoPermission as i32
    );

    let conditional = check_permissionship(
        &client,
        tuples.post_one,
        "view_as_fan",
        tuples.beatrice,
        None,
    )
    .await;
    assert_eq!(
        conditional.permissionship,
        check_permission_response::Permissionship::ConditionalPermission as i32
    );
    assert!(
        conditional
            .partial_caveat_info
            .as_ref()
            .is_some_and(|info| info.missing_required_context.contains(&"likes".to_string()))
    );
}

#[tokio::test]
#[serial]
async fn lookup_resources_returns_expected_items() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "lookup-resources").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let mut stream = client
        .permissions()
        .lookup_resources(LookupResourcesRequest {
            consistency: Some(fully_consistent()),
            resource_object_type: "test/post".to_string(),
            permission: "write".to_string(),
            subject: Some(tuples.emilia),
            ..Default::default()
        })
        .await
        .expect("lookup resources should succeed")
        .into_inner();

    let mut returned_ids = HashSet::new();
    while let Some(item) = stream
        .message()
        .await
        .expect("stream read should not error")
    {
        returned_ids.insert(item.resource_object_id);
    }

    assert!(returned_ids.contains(&tuples.post_one.object_id));
    assert!(returned_ids.contains(&tuples.post_two.object_id));
    assert_eq!(returned_ids.len(), 2);
}

#[tokio::test]
#[serial]
async fn lookup_subjects_returns_expected_items() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "lookup-subjects").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let emilia_id = tuples
        .emilia
        .object
        .as_ref()
        .expect("subject object should exist")
        .object_id
        .clone();
    let beatrice_id = tuples
        .beatrice
        .object
        .as_ref()
        .expect("subject object should exist")
        .object_id
        .clone();

    let mut stream = client
        .permissions()
        .lookup_subjects(LookupSubjectsRequest {
            consistency: Some(fully_consistent()),
            resource: Some(tuples.post_one),
            permission: "view".to_string(),
            subject_object_type: "test/user".to_string(),
            optional_subject_relation: String::new(),
            context: None,
            optional_concrete_limit: 0,
            optional_cursor: None,
            wildcard_option: 0,
        })
        .await
        .expect("lookup subjects should succeed")
        .into_inner();

    let mut subject_ids = HashSet::new();
    while let Some(item) = stream
        .message()
        .await
        .expect("stream read should not error")
    {
        let resolved = item.subject.expect("resolved subject should exist");
        subject_ids.insert(resolved.subject_object_id);
    }

    assert!(subject_ids.contains(&emilia_id));
    assert!(subject_ids.contains(&beatrice_id));
    assert_eq!(subject_ids.len(), 2);
}

#[tokio::test]
#[serial]
async fn read_relationships_returns_expected_items() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "read-relationships").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let mut stream = client
        .permissions()
        .read_relationships(ReadRelationshipsRequest {
            consistency: Some(fully_consistent()),
            relationship_filter: Some(RelationshipFilter {
                resource_type: "test/post".to_string(),
                optional_resource_id: tuples.post_one.object_id.clone(),
                optional_resource_id_prefix: String::new(),
                optional_relation: String::new(),
                optional_subject_filter: None,
            }),
            optional_limit: 0,
            optional_cursor: None,
        })
        .await
        .expect("read relationships should succeed")
        .into_inner();

    let mut relationships = Vec::new();
    while let Some(item) = stream
        .message()
        .await
        .expect("stream read should not error")
    {
        relationships.push(item.relationship.expect("relationship should exist"));
    }

    assert_eq!(relationships.len(), 3);
    let mut relations = HashSet::new();
    for rel in &relationships {
        assert_eq!(
            rel.resource
                .as_ref()
                .expect("resource should exist")
                .object_id,
            tuples.post_one.object_id
        );
        relations.insert(rel.relation.clone());
    }
    assert!(relations.contains("writer"));
    assert!(relations.contains("reader"));
    assert!(relations.contains("caveated_reader"));
}

#[tokio::test]
#[serial]
async fn check_bulk_permissions_returns_expected_pairs() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "check-bulk").await;
    write_test_schema(&client).await;
    let tuples = write_test_tuples(&client).await;

    let response = client
        .permissions()
        .check_bulk_permissions(CheckBulkPermissionsRequest {
            consistency: Some(fully_consistent()),
            items: vec![
                CheckBulkPermissionsRequestItem {
                    resource: Some(tuples.post_one.clone()),
                    permission: "view".to_string(),
                    subject: Some(tuples.emilia.clone()),
                    context: None,
                },
                CheckBulkPermissionsRequestItem {
                    resource: Some(tuples.post_one),
                    permission: "write".to_string(),
                    subject: Some(tuples.emilia),
                    context: None,
                },
            ],
            with_tracing: false,
        })
        .await
        .expect("check bulk permissions should succeed")
        .into_inner();

    assert_eq!(response.pairs.len(), 2);

    for pair in response.pairs {
        match pair.response.expect("pair response should exist") {
            check_bulk_permissions_pair::Response::Item(item) => {
                assert_eq!(
                    item.permissionship,
                    check_permission_response::Permissionship::HasPermission as i32
                );
            }
            check_bulk_permissions_pair::Response::Error(err) => {
                panic!("unexpected bulk check error: {}", err.message);
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn export_import_bulk_relationships_roundtrip() {
    let server = SpiceDbServer::start();
    let source_client = test_client(&server, "bulk-export-source").await;
    write_test_schema(&source_client).await;
    write_test_tuples(&source_client).await;

    let mut export_stream = source_client
        .permissions()
        .export_bulk_relationships(ExportBulkRelationshipsRequest {
            consistency: Some(fully_consistent()),
            optional_limit: 0,
            optional_cursor: None,
            optional_relationship_filter: None,
        })
        .await
        .expect("export bulk relationships should succeed")
        .into_inner();

    let mut exported = Vec::new();
    while let Some(page) = export_stream
        .message()
        .await
        .expect("export stream read should not error")
    {
        exported.extend(page.relationships);
    }
    assert_eq!(exported.len(), 4);

    let destination_client =
        test_client_with_token(&server, &unique_id("bulk-export-destination")).await;
    write_test_schema(&destination_client).await;

    let import_response = destination_client
        .permissions()
        .import_bulk_relationships(iter(vec![ImportBulkRelationshipsRequest {
            relationships: exported.clone(),
        }]))
        .await
        .expect("import bulk relationships should succeed")
        .into_inner();
    assert_eq!(import_response.num_loaded, exported.len() as u64);

    let mut verify_stream = destination_client
        .permissions()
        .export_bulk_relationships(ExportBulkRelationshipsRequest {
            consistency: Some(fully_consistent()),
            optional_limit: 0,
            optional_cursor: None,
            optional_relationship_filter: None,
        })
        .await
        .expect("verify export bulk relationships should succeed")
        .into_inner();

    let mut imported = Vec::new();
    while let Some(page) = verify_stream
        .message()
        .await
        .expect("verify export stream read should not error")
    {
        imported.extend(page.relationships);
    }

    assert_eq!(imported.len(), exported.len());
}

#[tokio::test]
#[serial]
async fn write_relationships_accepts_transaction_metadata() {
    let server = SpiceDbServer::start();
    let client = test_client(&server, "transaction-metadata").await;
    write_test_schema(&client).await;

    let resource = ObjectReference {
        object_type: "test/post".to_string(),
        object_id: unique_id("post"),
    };
    let subject = SubjectReference {
        object: Some(ObjectReference {
            object_type: "test/user".to_string(),
            object_id: unique_id("user"),
        }),
        optional_relation: String::new(),
    };

    let response = client
        .permissions()
        .write_relationships(WriteRelationshipsRequest {
            updates: vec![RelationshipUpdate {
                operation: relationship_update::Operation::Create as i32,
                relationship: Some(Relationship {
                    resource: Some(resource),
                    relation: "reader".to_string(),
                    subject: Some(subject),
                    optional_caveat: None,
                    optional_expires_at: None,
                }),
            }],
            optional_transaction_metadata: Some(string_struct(&[
                ("transaction_id", "tx-123"),
                ("other_data", "sample"),
            ])),
            ..Default::default()
        })
        .await
        .expect("write relationships with metadata should succeed")
        .into_inner();

    assert!(response.written_at.is_some());
}
