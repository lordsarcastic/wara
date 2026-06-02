use crate::{libs::docker::ProxyKind, models::domains::Domain};

pub fn generate_config(domain: &Domain, upstream: &str) -> String {
    match domain.proxy {
        ProxyKind::Nginx => format!(
            "server {{\n    listen 80;\n    server_name {};\n\n    location / {{\n        proxy_pass http://{};\n        proxy_set_header Host $host;\n        proxy_set_header X-Forwarded-Proto $scheme;\n    }}\n}}\n",
            domain.hostname, upstream
        ),
        ProxyKind::Traefik => format!(
            "http:\n  routers:\n    {}:\n      rule: Host(`{}`)\n      service: {}\n  services:\n    {}:\n      loadBalancer:\n        servers:\n          - url: http://{}\n",
            domain.id, domain.hostname, domain.service_id, domain.service_id, upstream
        ),
    }
}
