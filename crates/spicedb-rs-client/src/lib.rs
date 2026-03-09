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
        // Token is optional to support test/local environments or callers that inject auth
        // outside this client.
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

    /// Sets the Bearer token attached as `authorization` metadata.
    ///
    /// If omitted, requests are sent without an authorization header.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Toggles plaintext HTTP/2 transport (no TLS).
    ///
    /// This is typically useful for local development and integration tests.
    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    /// Eagerly connects to SpiceDB and fails fast on transport errors.
    pub async fn connect(self) -> Result<Client, ClientError> {
        let endpoint = endpoint_with_scheme(&self.endpoint, self.insecure);
        let channel = Endpoint::from_shared(endpoint)?.connect().await?;
        build_client(channel, self.token.as_deref())
    }

    /// Builds a client lazily, matching official clients that dial on first request.
    pub fn connect_lazy(self) -> Result<Client, ClientError> {
        let endpoint = endpoint_with_scheme(&self.endpoint, self.insecure);
        let channel = Endpoint::from_shared(endpoint)?.connect_lazy();
        build_client(channel, self.token.as_deref())
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

fn build_client(channel: Channel, token: Option<&str>) -> Result<Client, ClientError> {
    let interceptor = AuthInterceptor::from_token(token)?;

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

    /// Returns a cloned permissions client handle.
    pub fn permissions(&self) -> PermissionsServiceClient<InterceptedChannel> {
        self.permissions.clone()
    }

    /// Returns a cloned schema client handle.
    pub fn schema(&self) -> SchemaServiceClient<InterceptedChannel> {
        self.schema.clone()
    }

    /// Returns a cloned watch client handle.
    pub fn watch(&self) -> WatchServiceClient<InterceptedChannel> {
        self.watch.clone()
    }

    /// Returns a cloned experimental client handle.
    pub fn experimental(&self) -> ExperimentalServiceClient<InterceptedChannel> {
        self.experimental.clone()
    }

    /// Returns a cloned materialize watch-permissions client handle.
    pub fn watch_permissions(&self) -> WatchPermissionsServiceClient<InterceptedChannel> {
        self.watch_permissions.clone()
    }

    /// Returns a cloned materialize watch-permission-sets client handle.
    pub fn watch_permission_sets(&self) -> WatchPermissionSetsServiceClient<InterceptedChannel> {
        self.watch_permission_sets.clone()
    }
}
