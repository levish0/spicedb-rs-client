#![allow(clippy::large_enum_variant)]

pub mod authzed {
    pub mod api {
        pub mod materialize {
            pub mod v0 {
                tonic::include_proto!("authzed.api.materialize.v0");
            }
        }

        pub mod v1 {
            tonic::include_proto!("authzed.api.v1");
        }
    }
}

pub mod google {
    pub mod rpc {
        tonic::include_proto!("google.rpc");
    }
}
