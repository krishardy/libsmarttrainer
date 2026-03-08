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

use std::collections::BTreeSet;
use std::pin::Pin;

use async_trait::async_trait;
use btleplug::api::{
    Characteristic, PeripheralProperties, ScanFilter, Service, ValueNotification, WriteType,
};
use futures::Stream;

/// Trait abstraction over a BLE peripheral device.
///
/// This mirrors the subset of `btleplug::api::Peripheral` that we use,
/// enabling mock-based testing without hardware.
#[async_trait]
pub trait BlePeripheral: Send + Sync + 'static {
    async fn connect(&self) -> Result<(), btleplug::Error>;
    async fn disconnect(&self) -> Result<(), btleplug::Error>;
    async fn is_connected(&self) -> Result<bool, btleplug::Error>;
    async fn discover_services(&self) -> Result<(), btleplug::Error>;
    fn characteristics(&self) -> BTreeSet<Characteristic>;
    fn services(&self) -> BTreeSet<Service>;
    async fn properties(&self) -> Result<Option<PeripheralProperties>, btleplug::Error>;
    async fn read(&self, characteristic: &Characteristic) -> Result<Vec<u8>, btleplug::Error>;
    async fn write(
        &self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: WriteType,
    ) -> Result<(), btleplug::Error>;
    async fn subscribe(&self, characteristic: &Characteristic) -> Result<(), btleplug::Error>;
    async fn notifications(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>, btleplug::Error>;
}

/// Trait abstraction over a BLE adapter (central).
///
/// This mirrors the subset of `btleplug::api::Central` that we use,
/// enabling mock-based testing without hardware.
#[async_trait]
pub trait BleAdapter: Send + Sync {
    type Peripheral: BlePeripheral;

    async fn start_scan(&self, filter: ScanFilter) -> Result<(), btleplug::Error>;
    async fn stop_scan(&self) -> Result<(), btleplug::Error>;
    async fn peripherals(&self) -> Result<Vec<Self::Peripheral>, btleplug::Error>;
}

/// Newtype wrapper around `btleplug::platform::Peripheral`.
///
/// Delegates all trait methods to the underlying btleplug implementation.
/// No test coverage needed — pure hardware delegation.
pub struct BtleplugPeripheral(pub btleplug::platform::Peripheral);

#[async_trait]
impl BlePeripheral for BtleplugPeripheral {
    async fn connect(&self) -> Result<(), btleplug::Error> {
        btleplug::api::Peripheral::connect(&self.0).await
    }

    async fn disconnect(&self) -> Result<(), btleplug::Error> {
        btleplug::api::Peripheral::disconnect(&self.0).await
    }

    async fn is_connected(&self) -> Result<bool, btleplug::Error> {
        btleplug::api::Peripheral::is_connected(&self.0).await
    }

    async fn discover_services(&self) -> Result<(), btleplug::Error> {
        btleplug::api::Peripheral::discover_services(&self.0).await
    }

    fn characteristics(&self) -> BTreeSet<Characteristic> {
        btleplug::api::Peripheral::characteristics(&self.0)
    }

    fn services(&self) -> BTreeSet<Service> {
        btleplug::api::Peripheral::services(&self.0)
    }

    async fn properties(&self) -> Result<Option<PeripheralProperties>, btleplug::Error> {
        btleplug::api::Peripheral::properties(&self.0).await
    }

    async fn read(&self, characteristic: &Characteristic) -> Result<Vec<u8>, btleplug::Error> {
        btleplug::api::Peripheral::read(&self.0, characteristic).await
    }

    async fn write(
        &self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: WriteType,
    ) -> Result<(), btleplug::Error> {
        btleplug::api::Peripheral::write(&self.0, characteristic, data, write_type).await
    }

    async fn subscribe(&self, characteristic: &Characteristic) -> Result<(), btleplug::Error> {
        btleplug::api::Peripheral::subscribe(&self.0, characteristic).await
    }

    async fn notifications(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>, btleplug::Error> {
        btleplug::api::Peripheral::notifications(&self.0).await
    }
}

/// Newtype wrapper around `btleplug::platform::Adapter`.
///
/// Delegates all trait methods to the underlying btleplug implementation,
/// wrapping returned peripherals in `BtleplugPeripheral`.
/// No test coverage needed — pure hardware delegation.
pub struct BtleplugAdapter(pub btleplug::platform::Adapter);

#[async_trait]
impl BleAdapter for BtleplugAdapter {
    type Peripheral = BtleplugPeripheral;

    async fn start_scan(&self, filter: ScanFilter) -> Result<(), btleplug::Error> {
        btleplug::api::Central::start_scan(&self.0, filter).await
    }

    async fn stop_scan(&self) -> Result<(), btleplug::Error> {
        btleplug::api::Central::stop_scan(&self.0).await
    }

    async fn peripherals(&self) -> Result<Vec<BtleplugPeripheral>, btleplug::Error> {
        let peripherals = btleplug::api::Central::peripherals(&self.0).await?;
        Ok(peripherals.into_iter().map(BtleplugPeripheral).collect())
    }
}
