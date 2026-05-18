//! Policy parser for regulatory documents
//!
//! Parses structured regulatory documents (JSON/YAML representation of
//! FCC, ITU, and country-specific rules) into the internal rule representation.

use crate::rules::{BandRules, FrequencyBand, LicenseRequirement, PowerLimit, RegulatoryPolicy};
use ground_core::{Frequency, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Policy document parser
pub struct PolicyParser;

impl PolicyParser {
    /// Parse a JSON policy document
    pub fn parse_json(json: &str) -> Result<RegulatoryPolicy> {
        let value: Value = serde_json::from_str(json)
            .map_err(|e| ground_core::GroundStationError::InvalidConfiguration(format!("JSON parse error: {}", e)))?;
        
        Self::parse_value(&value)
    }
    
    /// Parse from a serde Value
    pub fn parse_value(value: &Value) -> Result<RegulatoryPolicy> {
        let name = value["name"]
            .as_str()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing policy name".to_string()))?
            .to_string();
        
        let jurisdiction = value["jurisdiction"]
            .as_str()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing jurisdiction".to_string()))?
            .to_string();
        
        let mut policy = RegulatoryPolicy::new(name, jurisdiction);
        
        if let Some(version) = value["version"].as_str() {
            policy.version = version.to_string();
        }
        
        if let Some(bands) = value["band_rules"].as_array() {
            for band_value in bands {
                let rules = Self::parse_band_rules(band_value)?;
                policy.add_band_rules(rules);
            }
        }
        
        Ok(policy)
    }
    
    /// Parse band rules
    fn parse_band_rules(value: &Value) -> Result<BandRules> {
        let band = Self::parse_frequency_band(value)?;
        let power_limits = Self::parse_power_limits(value)?;
        let license_requirement = Self::parse_license_requirement(value)?;
        let coordination = Self::parse_coordination_requirement(value)?;
        
        let additional_constraints = Vec::new(); // TODO: parse additional constraints
        
        Ok(BandRules {
            band,
            power_limits,
            license_requirement,
            coordination,
            additional_constraints,
        })
    }
    
    /// Parse frequency band
    fn parse_frequency_band(value: &Value) -> Result<FrequencyBand> {
        let name = value["band"]["name"]
            .as_str()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing band name".to_string()))?
            .to_string();
        
        let min_frequency = value["band"]["min_frequency"]
            .as_u64()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing min frequency".to_string()))?;
        
        let max_frequency = value["band"]["max_frequency"]
            .as_u64()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing max frequency".to_string()))?;
        
        let regulatory_body = value["band"]["regulatory_body"]
            .as_str()
            .unwrap_or("UNKNOWN")
            .to_string();
        
        let jurisdiction = value["band"]["jurisdiction"]
            .as_str()
            .unwrap_or("GLOBAL")
            .to_string();
        
        Ok(FrequencyBand {
            name,
            lower_hz: min_frequency,
            upper_hz: max_frequency,
            regulatory_body,
            jurisdiction,
        })
    }
    
    /// Parse power limits
    fn parse_power_limits(value: &Value) -> Result<PowerLimit> {
        let max_power_dbm = value["power_limits"]["max_power_dbm"]
            .as_f64()
            .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing max power".to_string()))?;
        
        let power_type_str = value["power_limits"]["power_type"]
            .as_str()
            .unwrap_or("eirp");
        
        let power_type = match power_type_str.to_lowercase().as_str() {
            "eirp" => crate::rules::PowerType::Eirp,
            "conducted" => crate::rules::PowerType::Conducted,
            _ => return Err(ground_core::GroundStationError::InvalidConfiguration(format!("Unknown power type: {}", power_type_str))),
        };
        
        let constraints = Vec::new(); // TODO: parse constraints
        
        Ok(PowerLimit {
            max_power_dbm,
            power_type,
            constraints,
        })
    }
    
    /// Parse license requirement
    fn parse_license_requirement(value: &Value) -> Result<LicenseRequirement> {
        let license_type_str = value["license_requirement"]["license_type"]
            .as_str()
            .unwrap_or("none");
        
        let license_type = match license_type_str.to_lowercase().as_str() {
            "none" => crate::rules::LicenseType::None,
            "general" => crate::rules::LicenseType::General,
            "amateur" => {
                let class = value["license_requirement"]["class"]
                    .as_str()
                    .unwrap_or("technician");
                crate::rules::LicenseType::Amateur { class: class.to_string() }
            }
            "specific" => {
                let license_class = value["license_requirement"]["license_class"]
                    .as_str()
                    .ok_or_else(|| ground_core::GroundStationError::InvalidConfiguration("Missing license class".to_string()))?;
                crate::rules::LicenseType::Specific { license_class: license_class.to_string() }
            }
            _ => return Err(ground_core::GroundStationError::InvalidConfiguration(format!("Unknown license type: {}", license_type_str))),
        };
        
        let requires_coordination = value["license_requirement"]["requires_coordination"]
            .as_bool()
            .unwrap_or(false);
        
        let validity_period = None; // TODO: parse validity period
        
        Ok(LicenseRequirement {
            license_type,
            requires_coordination,
            validity_period,
        })
    }
    
    /// Parse coordination requirement
    fn parse_coordination_requirement(value: &Value) -> Result<crate::rules::CoordinationRequirement> {
        let coordination_bodies = value["coordination"]["bodies"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        
        let process_str = value["coordination"]["process"]
            .as_str()
            .unwrap_or("none");
        
        let process = match process_str.to_lowercase().as_str() {
            "none" => crate::rules::CoordinationProcess::None,
            "dynamic" => {
                let response_time = value["coordination"]["response_time_ms"]
                    .as_u64()
                    .unwrap_or(100);
                crate::rules::CoordinationProcess::Dynamic { response_time_ms: response_time }
            }
            "pre" => {
                let notice_days = value["coordination"]["notice_days"]
                    .as_u64()
                    .unwrap_or(30);
                crate::rules::CoordinationProcess::PreCoordination { notice_days }
            }
            _ => return Err(ground_core::GroundStationError::InvalidConfiguration(format!("Unknown coordination process: {}", process_str))),
        };
        
        Ok(crate::rules::CoordinationRequirement {
            coordination_bodies,
            process,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_policy() {
        let json = r#"
        {
            "name": "Test Policy",
            "jurisdiction": "US",
            "version": "1.0",
            "band_rules": [
                {
                    "band": {
                        "name": "L-band",
                        "min_frequency": 1000000000,
                        "max_frequency": 2000000000,
                        "regulatory_body": "FCC",
                        "jurisdiction": "US"
                    },
                    "power_limits": {
                        "max_power_dbm": 30.0,
                        "power_type": "eirp"
                    },
                    "license_requirement": {
                        "license_type": "general",
                        "requires_coordination": false
                    },
                    "coordination": {
                        "bodies": [],
                        "process": "none"
                    }
                }
            ]
        }
        "#;
        
        let policy = PolicyParser::parse_json(json).unwrap();
        assert_eq!(policy.name, "Test Policy");
        assert_eq!(policy.band_rules.len(), 1);
        assert_eq!(policy.band_rules[0].band.name, "L-band");
    }
}
