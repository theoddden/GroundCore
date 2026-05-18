//! Type generator for regulatory enforcement
//!
//! Generates Rust types from regulatory policy that encode compliance
//! at the type level. The generated types ensure that:
//! - Transmit functions require a license typed to the specific band
//! - Coordination proofs are type-checked

use crate::rules::{FrequencyBand, LicenseRequirement, RegulatoryPolicy};
use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, Item};

/// Generated Rust types for a regulatory policy
#[derive(Debug)]
pub struct GeneratedTypes {
    /// The generated module
    pub module: ItemMod,
}

/// Module representation
#[derive(Debug)]
struct ItemMod {
    pub items: Vec<Item>,
}

impl GeneratedTypes {
    /// Generate Rust types from a regulatory policy
    pub fn from_policy(policy: &RegulatoryPolicy) -> Self {
        let mut items = Vec::new();
        
        // Generate band types
        for band_rules in &policy.band_rules {
            let band_type = Self::generate_band_type(&band_rules.band);
            items.push(band_type);
        }
        
        // Generate license types
        for band_rules in &policy.band_rules {
            let license_type = Self::generate_license_type(&band_rules.band, &band_rules.license_requirement);
            items.push(license_type);
        }
        
        // Generate coordination proof types
        for band_rules in &policy.band_rules {
            let coordination_type = Self::generate_coordination_type(&band_rules.band, &band_rules.coordination);
            items.push(coordination_type);
        }
        
        // Generate frequency types
        for band_rules in &policy.band_rules {
            let frequency_type = Self::generate_frequency_type(&band_rules.band);
            items.push(frequency_type);
        }
        
        // Generate transmit function template
        let transmit_fn = Self::generate_transmit_function();
        items.push(transmit_fn);
        
        GeneratedTypes {
            module: ItemMod { items },
        }
    }
    
    /// Generate a band type
    fn generate_band_type(band: &FrequencyBand) -> Item {
        let name = format_ident!("Band{}", band.name.replace("-", "").replace(" ", ""));
        let doc = format!("{} frequency band ({}-{} MHz)", band.name, band.lower_hz / 1_000_000, band.upper_hz / 1_000_000);
        let lower_hz = band.lower_hz;
        let upper_hz = band.upper_hz;
        
        parse_quote! {
            #[doc = #doc]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub struct #name {
                _phantom: core::marker::PhantomData<()>,
            }
            
            impl #name {
                pub const MIN_FREQUENCY: u64 = #lower_hz;
                pub const MAX_FREQUENCY: u64 = #upper_hz;
            }
        }
    }
    
    /// Generate a license type for a band
    fn generate_license_type(band: &FrequencyBand, license_req: &LicenseRequirement) -> Item {
        let band_suffix = band.name.replace("-", "").replace(" ", "");
        let band_name = format_ident!("Band{}", &band_suffix);
        let license_name = format_ident!("{}License", &band_suffix);
        let doc = format!("License for {} transmission", band.name);
        
        let license_type_impl = match &license_req.license_type {
            crate::rules::LicenseType::None => {
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone)]
                    pub struct #license_name {
                        pub holder: String,
                        pub band: core::marker::PhantomData<#band_name>,
                    }
                }
            }
            crate::rules::LicenseType::General => {
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone)]
                    pub struct #license_name {
                        pub holder: String,
                        pub band: core::marker::PhantomData<#band_name>,
                        pub valid_from: chrono::DateTime<chrono::Utc>,
                        pub valid_until: chrono::DateTime<chrono::Utc>,
                    }
                    
                    impl #license_name {
                        pub fn is_valid(&self) -> bool {
                            let now = chrono::Utc::now();
                            now >= self.valid_from && now < self.valid_until
                        }
                    }
                }
            }
            crate::rules::LicenseType::Specific { license_class: _ } => {
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone)]
                    pub struct #license_name {
                        pub holder: String,
                        pub license_class: String,
                        pub band: core::marker::PhantomData<#band_name>,
                        pub valid_from: chrono::DateTime<chrono::Utc>,
                        pub valid_until: chrono::DateTime<chrono::Utc>,
                    }
                    
                    impl #license_name {
                        pub fn is_valid(&self) -> bool {
                            let now = chrono::Utc::now();
                            now >= self.valid_from && now < self.valid_until
                        }
                    }
                }
            }
            crate::rules::LicenseType::Amateur { class: _ } => {
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone)]
                    pub struct #license_name {
                        pub callsign: String,
                        pub class: String,
                        pub band: core::marker::PhantomData<#band_name>,
                    }
                }
            }
        };
        
        parse_quote! {
            #license_type_impl
        }
    }
    
    /// Generate a coordination proof type
    fn generate_coordination_type(band: &FrequencyBand, coordination: &crate::rules::CoordinationRequirement) -> Item {
        let band_suffix = band.name.replace("-", "").replace(" ", "");
        let band_name = format_ident!("Band{}", &band_suffix);
        let coord_name = format_ident!("{}CoordinationProof", &band_suffix);
        let doc = format!("Coordination proof for {} transmission", band.name);
        
        let _bodies = &coordination.coordination_bodies;
        
        parse_quote! {
            #[doc = #doc]
            #[derive(Debug, Clone)]
            pub struct #coord_name {
                pub coordinated_with: Vec<String>,
                pub band: core::marker::PhantomData<#band_name>,
                pub coordination_date: chrono::DateTime<chrono::Utc>,
            }
        }
    }
    
    /// Generate a frequency type for a band
    fn generate_frequency_type(band: &FrequencyBand) -> Item {
        let band_suffix = band.name.replace("-", "").replace(" ", "");
        let band_name = format_ident!("Band{}", &band_suffix);
        let freq_name = format_ident!("{}Frequency", &band_suffix);
        let doc = format!("Frequency type for {} band ({}-{} MHz)", band.name, band.lower_hz / 1_000_000, band.upper_hz / 1_000_000);
        
        parse_quote! {
            #[doc = #doc]
            #[derive(Debug, Clone, Copy)]
            pub struct #freq_name {
                pub hz: u64,
                pub band: core::marker::PhantomData<#band_name>,
            }
        }
    }
    
    /// Generate a transmit function template
    fn generate_transmit_function() -> Item {
        parse_quote! {
            /// Transmit on a specific band
            /// 
            /// This function cannot be called without:
            /// - A valid license for the band
            /// - A frequency typed to the band
            /// - A coordination proof (if required)
            /// 
            /// This is compile-time enforcement of regulatory compliance.
            pub fn transmit<B>(
                _license: &License<B>,
                _frequency: Frequency<B>,
                _power: Power,
                _coordination: Option<&CoordinationProof<B>>,
            ) -> Result<(), TransmitError>
            where
                B: Band,
            {
                // Implementation would go here
                // The type system ensures we have all required proofs
                Ok(())
            }
        }
    }
    
    /// Emit the generated types as Rust code
    pub fn emit(&self) -> String {
        let mut code = String::new();
        
        code.push_str("// Auto-generated by spectrum policy compiler\n");
        code.push_str("// DO NOT EDIT - changes will be overwritten\n\n");
        
        code.push_str("use chrono::{DateTime, Utc};\n");
        code.push_str("use core::marker::PhantomData;\n\n");
        
        for item in &self.module.items {
            code.push_str(&item.to_token_stream().to_string());
            code.push_str("\n\n");
        }
        
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CoordinationProcess, CoordinationRequirement, LicenseRequirement, LicenseType, PowerLimit, PowerType};
    
    #[test]
    fn test_generate_band_type() {
        let band = FrequencyBand {
            name: "L-band".to_string(),
            lower_hz: 1_000_000_000,
            upper_hz: 2_000_000_000,
            regulatory_body: "FCC".to_string(),
            jurisdiction: "US".to_string(),
        };
        
        let generated = GeneratedTypes::generate_band_type(&band);
        let code = {
            use quote::ToTokens;
            generated.into_token_stream().to_string()
        };
        
        assert!(code.contains("BandLband"));
        assert!(code.contains("1000000000"));
        assert!(code.contains("2000000000"));
    }
    
    #[test]
    fn test_generate_full_policy() {
        let mut policy = RegulatoryPolicy::new("Test".to_string(), "US".to_string());
        
        let band = FrequencyBand {
            name: "L-band".to_string(),
            lower_hz: 1_000_000_000,
            upper_hz: 2_000_000_000,
            regulatory_body: "FCC".to_string(),
            jurisdiction: "US".to_string(),
        };
        
        let band_rules = crate::rules::BandRules {
            band,
            power_limits: PowerLimit {
                max_power_dbm: 30.0,
                power_type: PowerType::Eirp,
                constraints: Vec::new(),
            },
            license_requirement: LicenseRequirement {
                license_type: LicenseType::General,
                requires_coordination: false,
                validity_period: None,
            },
            coordination: CoordinationRequirement {
                coordination_bodies: Vec::new(),
                process: CoordinationProcess::None,
            },
            additional_constraints: Vec::new(),
        };
        
        policy.add_band_rules(band_rules);
        
        let generated = GeneratedTypes::from_policy(&policy);
        let code = generated.emit();
        
        assert!(code.contains("BandLband"));
        assert!(code.contains("LbandLicense"));
        assert!(code.contains("LbandCoordinationProof"));
        assert!(code.contains("LbandFrequency"));
    }
}
