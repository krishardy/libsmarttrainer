// libsmarttrainer - Library for controlling a bicycle smart trainer
// Copyright (C) 2026 Kris Hardy <hardyrk@gmail.com>
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public
// License along with this library; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301
// USA

use std::time::Duration;

use btleplug::api::{Manager as _, ScanFilter};

use crate::ble::constants::FTMS_SERVICE_UUID;
use crate::ble::error::{BleTransportError, Result};
use crate::ble::traits::{BleAdapter, BlePeripheral, BtleplugAdapter};

/// A discovered FTMS device with its name and address.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub address: String,
}

/// Scan for FTMS devices for the given duration.
///
/// Returns all discovered peripherals advertising the FTMS service (0x1826),
/// paired with their `DiscoveredDevice` metadata.
pub async fn scan_for_ftms_devices<A: BleAdapter>(
    adapter: &A,
    timeout: Duration,
) -> Result<Vec<(DiscoveredDevice, A::Peripheral)>> {
    let scan_filter = ScanFilter {
        services: vec![FTMS_SERVICE_UUID],
    };
    adapter.start_scan(scan_filter).await?;

    tokio::time::sleep(timeout).await;

    adapter.stop_scan().await?;

    let peripherals = adapter.peripherals().await?;
    let mut devices = Vec::new();

    for p in peripherals {
        let props = p.properties().await?.unwrap_or_default();
        let has_ftms = props.services.contains(&FTMS_SERVICE_UUID);
        if has_ftms {
            let name = props
                .local_name
                .unwrap_or_else(|| "Unknown".to_string());
            let address = props.address.to_string();
            devices.push((DiscoveredDevice { name, address }, p));
        }
    }

    Ok(devices)
}

/// Returns the first available Bluetooth adapter, wrapped as a `BtleplugAdapter`.
pub async fn get_adapter() -> Result<BtleplugAdapter> {
    let manager = btleplug::platform::Manager::new().await?;
    let adapters = manager.adapters().await?;
    adapters
        .into_iter()
        .next()
        .map(BtleplugAdapter)
        .ok_or(BleTransportError::NoAdapter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::traits::BlePeripheral;
    use async_trait::async_trait;
    use btleplug::api::{
        Characteristic, PeripheralProperties, ScanFilter, Service, ValueNotification, WriteType,
    };
    use std::collections::BTreeSet;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    // --- Mock Peripheral ---

    struct MockPeripheral {
        props: Option<PeripheralProperties>,
    }

    #[async_trait]
    impl BlePeripheral for MockPeripheral {
        async fn connect(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn disconnect(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn is_connected(&self) -> std::result::Result<bool, btleplug::Error> {
            Ok(false)
        }
        async fn discover_services(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        fn characteristics(&self) -> BTreeSet<Characteristic> {
            BTreeSet::new()
        }
        fn services(&self) -> BTreeSet<Service> {
            BTreeSet::new()
        }
        async fn properties(
            &self,
        ) -> std::result::Result<Option<PeripheralProperties>, btleplug::Error> {
            Ok(self.props.clone())
        }
        async fn read(
            &self,
            _characteristic: &Characteristic,
        ) -> std::result::Result<Vec<u8>, btleplug::Error> {
            Ok(vec![])
        }
        async fn write(
            &self,
            _characteristic: &Characteristic,
            _data: &[u8],
            _write_type: WriteType,
        ) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn subscribe(
            &self,
            _characteristic: &Characteristic,
        ) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn notifications(
            &self,
        ) -> std::result::Result<
            Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>,
            btleplug::Error,
        > {
            Ok(Box::pin(futures::stream::empty()))
        }
    }

    // --- Mock Adapter ---

    struct MockAdapter {
        peripherals: Vec<MockPeripheral>,
        scan_started: Arc<Mutex<bool>>,
        scan_stopped: Arc<Mutex<bool>>,
    }

    impl MockAdapter {
        fn new(peripherals: Vec<MockPeripheral>) -> Self {
            Self {
                peripherals,
                scan_started: Arc::new(Mutex::new(false)),
                scan_stopped: Arc::new(Mutex::new(false)),
            }
        }
    }

    #[async_trait]
    impl BleAdapter for MockAdapter {
        type Peripheral = MockPeripheral;

        async fn start_scan(
            &self,
            _filter: ScanFilter,
        ) -> std::result::Result<(), btleplug::Error> {
            *self.scan_started.lock().unwrap() = true;
            Ok(())
        }

        async fn stop_scan(&self) -> std::result::Result<(), btleplug::Error> {
            *self.scan_stopped.lock().unwrap() = true;
            Ok(())
        }

        async fn peripherals(
            &self,
        ) -> std::result::Result<Vec<MockPeripheral>, btleplug::Error> {
            // Return new peripherals with the same props (MockPeripheral is not Clone)
            Ok(self
                .peripherals
                .iter()
                .map(|p| MockPeripheral {
                    props: p.props.clone(),
                })
                .collect())
        }
    }

    fn make_ftms_props(name: &str) -> PeripheralProperties {
        PeripheralProperties {
            local_name: Some(name.to_string()),
            services: vec![FTMS_SERVICE_UUID],
            ..Default::default()
        }
    }

    fn make_non_ftms_props(name: &str) -> PeripheralProperties {
        PeripheralProperties {
            local_name: Some(name.to_string()),
            services: vec![],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn scan_finds_ftms_devices() {
        let adapter = MockAdapter::new(vec![
            MockPeripheral {
                props: Some(make_ftms_props("Trainer A")),
            },
            MockPeripheral {
                props: Some(make_non_ftms_props("Headphones")),
            },
            MockPeripheral {
                props: Some(make_ftms_props("Trainer B")),
            },
        ]);

        let results = scan_for_ftms_devices(&adapter, Duration::from_millis(1))
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.name, "Trainer A");
        assert_eq!(results[1].0.name, "Trainer B");
    }

    #[tokio::test]
    async fn scan_empty_results() {
        let adapter = MockAdapter::new(vec![]);
        let results = scan_for_ftms_devices(&adapter, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn scan_filters_non_ftms() {
        let adapter = MockAdapter::new(vec![MockPeripheral {
            props: Some(make_non_ftms_props("Not FTMS")),
        }]);
        let results = scan_for_ftms_devices(&adapter, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn scan_handles_none_properties() {
        let adapter = MockAdapter::new(vec![MockPeripheral { props: None }]);
        let results = scan_for_ftms_devices(&adapter, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn scan_uses_unknown_for_nameless_device() {
        let mut props = make_ftms_props("ignored");
        props.local_name = None;
        let adapter = MockAdapter::new(vec![MockPeripheral { props: Some(props) }]);
        let results = scan_for_ftms_devices(&adapter, Duration::from_millis(1))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name, "Unknown");
    }

    #[tokio::test]
    async fn scan_calls_start_and_stop() {
        let adapter = MockAdapter::new(vec![]);
        let _ = scan_for_ftms_devices(&adapter, Duration::from_millis(1)).await;
        assert!(*adapter.scan_started.lock().unwrap());
        assert!(*adapter.scan_stopped.lock().unwrap());
    }

    #[test]
    fn discovered_device_debug() {
        let dev = DiscoveredDevice {
            name: "Test".to_string(),
            address: "00:11:22:33:44:55".to_string(),
        };
        let debug = format!("{:?}", dev);
        assert!(debug.contains("Test"));
    }
}
