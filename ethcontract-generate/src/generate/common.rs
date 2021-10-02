use crate::generate::Context;
use crate::util::expand_doc;
use ethcontract_common::artifact::truffle::TruffleLoader;
use ethcontract_common::{Address, DeploymentInformation};
use proc_macro2::{Literal, TokenStream};
use quote::quote;

pub(crate) fn expand(cx: &Context) -> TokenStream {
    let contract_name = &cx.contract_name;

    let doc_str = cx
        .contract
        .devdoc
        .details
        .as_deref()
        .unwrap_or("Generated by `ethcontract`");
    let doc = expand_doc(doc_str);

    let contract_json = TruffleLoader::save_to_string(cx.contract).unwrap();

    let deployments = cx.networks.iter().map(|(chain_id, network)| {
        let chain_id = Literal::string(chain_id);
        let address = expand_address(network.address);
        let deployment_information = expand_deployment_information(network.deployment_information);

        quote! {
            contract.networks.insert(
                #chain_id.to_owned(),
                self::ethcontract::common::contract::Network {
                    address: #address,
                    deployment_information: #deployment_information,
                },
            );
        }
    });

    quote! {
        #doc
        #[derive(Clone)]
        pub struct Contract {
            methods: Methods,
        }

        impl Contract {
            /// Retrieves the raw contract instance used to generate the type safe
            /// API for this contract.
            pub fn raw_contract() -> &'static self::ethcontract::Contract {
                use self::ethcontract::common::artifact::truffle::TruffleLoader;
                use self::ethcontract::private::lazy_static;
                use self::ethcontract::Contract;

                lazy_static! {
                    pub static ref CONTRACT: Contract = {
                        #[allow(unused_mut)]
                        let mut contract = TruffleLoader::new()
                            .load_contract_from_str(#contract_json)
                            .expect("valid contract JSON");
                        #( #deployments )*

                        contract
                    };
                }
                &CONTRACT
            }

            /// Creates a new contract instance with the specified `web3`
            /// provider at the given `Address`.
            ///
            /// Note that this does not verify that a contract with a matching
            /// `Abi` is actually deployed at the given address.
            pub fn at<F, B, T>(
                web3: &self::ethcontract::web3::api::Web3<T>,
                address: self::ethcontract::Address,
            ) -> Self
            where
                F: std::future::Future<
                        Output = Result<
                            self::ethcontract::json::Value,
                            self::ethcontract::web3::Error,
                        >,
                    > + Send
                    + 'static,
                B: std::future::Future<
                        Output = Result<
                            Vec<
                                Result<
                                    self::ethcontract::json::Value,
                                    self::ethcontract::web3::Error,
                                >,
                            >,
                            self::ethcontract::web3::Error,
                        >,
                    > + Send
                    + 'static,
                T: self::ethcontract::web3::Transport<Out = F>
                    + self::ethcontract::web3::BatchTransport<Batch = B>
                    + Send
                    + Sync
                    + 'static,
            {
                Contract::with_deployment_info(web3, address, None)
            }

            /// Creates a new contract instance with the specified `web3` provider with
            /// the given `Abi` at the given `Address` and an optional transaction hash.
            /// This hash is used to retrieve contract related information such as the
            /// creation block (which is useful for fetching all historic events).
            ///
            /// Note that this does not verify that a contract with a matching `Abi` is
            /// actually deployed at the given address nor that the transaction hash,
            /// when provided, is actually for this contract deployment.
            pub fn with_deployment_info<F, B, T>(
                web3: &self::ethcontract::web3::api::Web3<T>,
                address: self::ethcontract::Address,
                deployment_information: Option<ethcontract::common::DeploymentInformation>,
            ) -> Self
            where
                F: std::future::Future<
                        Output = Result<
                            self::ethcontract::json::Value,
                            self::ethcontract::web3::Error,
                        >,
                    > + Send
                    + 'static,
                B: std::future::Future<
                        Output = Result<
                            Vec<
                                Result<
                                    self::ethcontract::json::Value,
                                    self::ethcontract::web3::Error,
                                >,
                            >,
                            self::ethcontract::web3::Error,
                        >,
                    > + Send
                    + 'static,
                T: self::ethcontract::web3::Transport<Out = F>
                    + self::ethcontract::web3::BatchTransport<Batch = B>
                    + Send
                    + Sync
                    + 'static,
            {
                use self::ethcontract::Instance;
                use self::ethcontract::transport::DynTransport;
                use self::ethcontract::web3::api::Web3;

                let transport = DynTransport::new(web3.transport().clone());
                let web3 = Web3::new(transport);
                let abi = Self::raw_contract().abi.clone();
                let instance = Instance::with_deployment_info(web3, abi, address, deployment_information);

                Contract::from_raw(instance)
            }

            /// Creates a contract from a raw instance.
            fn from_raw(instance: self::ethcontract::dyns::DynInstance) -> Self {
                let methods = Methods { instance };
                Contract { methods }
            }

            /// Returns the contract address being used by this instance.
            pub fn address(&self) -> self::ethcontract::Address {
                self.raw_instance().address()
            }

            /// Returns the deployment information of the contract
            /// if it is known, `None` otherwise.
            pub fn deployment_information(&self) -> Option<ethcontract::common::DeploymentInformation> {
                self.raw_instance().deployment_information()
            }

            /// Returns a reference to the default method options used by this
            /// contract.
            pub fn defaults(&self) -> &self::ethcontract::contract::MethodDefaults<self::ethcontract::transport::DynTransport>
            {
                &self.raw_instance().defaults
            }

            /// Returns a mutable reference to the default method options used
            /// by this contract.
            pub fn defaults_mut(&mut self) -> &mut self::ethcontract::contract::MethodDefaults<self::ethcontract::transport::DynTransport>
            {
                &mut self.raw_instance_mut().defaults
            }

            /// Returns a reference to the raw runtime instance used by this
            /// contract.
            pub fn raw_instance(&self) -> &self::ethcontract::dyns::DynInstance {
                &self.methods.instance
            }

            /// Returns a mutable reference to the raw runtime instance used by
            /// this contract.
            fn raw_instance_mut(&mut self) -> &mut self::ethcontract::dyns::DynInstance {
                &mut self.methods.instance
            }
        }

        impl std::fmt::Debug for Contract {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.debug_tuple(stringify!(#contract_name))
                    .field(&self.address())
                    .finish()
            }
        }
    }
}

/// Expands an `Address` into a literal representation that can be used with
/// quasi-quoting for code generation.
fn expand_address(address: Address) -> TokenStream {
    let bytes = address
        .as_bytes()
        .iter()
        .copied()
        .map(Literal::u8_unsuffixed);

    quote! {
        self::ethcontract::H160([#( #bytes ),*])
    }
}

/// Expands a deployment info into a literal representation that can be used
/// with quasi-quoting for code generation.
fn expand_deployment_information(deployment: Option<DeploymentInformation>) -> TokenStream {
    match deployment {
        Some(DeploymentInformation::BlockNumber(block)) => quote! {
            Some(ethcontract::common::DeploymentInformation::BlockNumber(#block))
        },
        Some(DeploymentInformation::TransactionHash(hash)) => {
            let bytes = hash.as_bytes().iter().copied().map(Literal::u8_unsuffixed);
            quote! {
                Some(ethcontract::common::DeploymentInformation::TransactionHash([#( #bytes ),*].into()))
            }
        }
        None => return quote! { None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn expand_address_value() {
        assert_quote!(
            expand_address(Address::zero()),
            {
                self::ethcontract::H160([ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ])
            },
        );

        assert_quote!(
            expand_address("000102030405060708090a0b0c0d0e0f10111213".parse().unwrap()),
            {
                self::ethcontract::H160([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19])
            },
        );
    }

    #[test]
    #[rustfmt::skip]
    fn expand_deployment_information_value() {
        assert_quote!(expand_deployment_information(None), { None });

        assert_quote!(
            expand_deployment_information(Some(DeploymentInformation::TransactionHash("000102030405060708090a0b0c0d0e0f10111213000000000000000000000000".parse().unwrap()))),
            {
                Some(ethcontract::common::DeploymentInformation::TransactionHash([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into()))
            },
        );

        assert_quote!(
            expand_deployment_information(Some(DeploymentInformation::BlockNumber(42))),
            {
                Some(ethcontract::common::DeploymentInformation::BlockNumber(42u64))
            },
        );


    }
}
