// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use crate::network::{
        network_model::{tc_filter_model::fetch_index, TC_FILTER_NET_MODEL_STR},
        network_pair::NetworkPair,
    };
    use anyhow::Context;
    use scopeguard::defer;
    #[actix_rt::test]
    async fn test_tc_redirect_network() {
        if let Ok((connection, handle, _)) = rtnetlink::new_connection().context("new connection") {
            let thread_handler = tokio::spawn(connection);
            defer!({
                thread_handler.abort();
            });

            handle
                .link()
                .add()
                .veth("foo".to_string(), "bar".to_string());

            if let Ok(net_pair) =
                NetworkPair::new(&handle, 1, "bar", TC_FILTER_NET_MODEL_STR, 2).await
            {
                if let Ok(index) = fetch_index(&handle, "bar").await {
                    assert!(net_pair.add_network_model().await.is_ok());
                    assert!(net_pair.del_network_model().await.is_ok());
                    assert!(handle.link().del(index).execute().await.is_ok());
                }
            }
        }
    }
}
