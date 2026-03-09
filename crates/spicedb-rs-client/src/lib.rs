use spicedb_rs_proto::authzed::api::{
    materialize::v0::{
        watch_permission_sets_service_client::WatchPermissionSetsServiceClient,
        watch_permissions_service_client::WatchPermissionsServiceClient,
    },
    v1::{
        experimental_service_client::ExperimentalServiceClient,
        permissions_service_client::PermissionsServiceClient,
        schema_service_client::SchemaServiceClient, watch_service_client::WatchServiceClient,
    },
};
use tonic::{
    Request, Status,
    metadata::{Ascii, MetadataValue, errors::InvalidMetadataValue},
    service::{Interceptor, interceptor::InterceptedService},
    transport::{Channel, Endpoint},
};

pub use spicedb_rs_proto::authzed::api::{materialize::v0, v1};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(#[from] http::uri::InvalidUri),
    #[error("invalid token metadata: {0}")]
    InvalidTokenMetadata(#[from] InvalidMetadataValue),
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct AuthInterceptor {
    authorization_header: Option<MetadataValue<Ascii>>,
}

impl AuthInterceptor {
    fn from_token(token: Option<&str>) -> Result<Self, InvalidMetadataValue> {
        let authorization_header = token
            .map(|token| format!("Bearer {token}").parse())
            .transpose()?;
        Ok(Self {
            authorization_header,
        })
    }
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if let Some(authorization_header) = &self.authorization_header {
            request
                .metadata_mut()
                .insert("authorization", authorization_header.clone());
        }
        Ok(request)
    }
}

pub type InterceptedChannel = InterceptedService<Channel, AuthInterceptor>;

#[derive(Debug, Clone)]
pub struct ClientBuilder {
    endpoint: String,
    token: Option<String>,
    insecure: bool,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            endpoint: "grpc.authzed.com:443".to_string(),
            token: None,
            insecure: false,
        }
    }
}

impl ClientBuilder {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            ..Self::default()
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    pub async fn connect(self) -> Result<Client, ClientError> {
        let endpoint = endpoint_with_scheme(&self.endpoint, self.insecure);
        let channel = Endpoint::from_shared(endpoint)?.connect().await?;
        let interceptor = AuthInterceptor::from_token(self.token.as_deref())?;

        Ok(Client {
            permissions: PermissionsServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            schema: SchemaServiceClient::with_interceptor(channel.clone(), interceptor.clone()),
            watch: WatchServiceClient::with_interceptor(channel.clone(), interceptor.clone()),
            experimental: ExperimentalServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            watch_permissions: WatchPermissionsServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            watch_permission_sets: WatchPermissionSetsServiceClient::with_interceptor(
                channel,
                interceptor,
            ),
        })
    }
}

fn endpoint_with_scheme(endpoint: &str, insecure: bool) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else if insecure {
        format!("http://{endpoint}")
    } else {
        format!("https://{endpoint}")
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    permissions: PermissionsServiceClient<InterceptedChannel>,
    schema: SchemaServiceClient<InterceptedChannel>,
    watch: WatchServiceClient<InterceptedChannel>,
    experimental: ExperimentalServiceClient<InterceptedChannel>,
    watch_permissions: WatchPermissionsServiceClient<InterceptedChannel>,
    watch_permission_sets: WatchPermissionSetsServiceClient<InterceptedChannel>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn permissions(&mut self) -> &mut PermissionsServiceClient<InterceptedChannel> {
        &mut self.permissions
    }

    pub fn schema(&mut self) -> &mut SchemaServiceClient<InterceptedChannel> {
        &mut self.schema
    }

    pub fn watch(&mut self) -> &mut WatchServiceClient<InterceptedChannel> {
        &mut self.watch
    }

    pub fn experimental(&mut self) -> &mut ExperimentalServiceClient<InterceptedChannel> {
        &mut self.experimental
    }

    pub fn watch_permissions(&mut self) -> &mut WatchPermissionsServiceClient<InterceptedChannel> {
        &mut self.watch_permissions
    }

    pub fn watch_permission_sets(
        &mut self,
    ) -> &mut WatchPermissionSetsServiceClient<InterceptedChannel> {
        &mut self.watch_permission_sets
    }
}
