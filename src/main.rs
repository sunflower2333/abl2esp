// Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
// SPDX-License-Identifier: BSD-3-Clause

#![no_main]
#![no_std]

use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};

use log::info;
use uefi::boot::{self, EventType, LoadImageSource, SearchType, Tpl};
use uefi::proto::device_path::{build, DevicePath};
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::BootPolicy;
use uefi::{guid, prelude::*, CStr16};
use uefi::{Identify, Result};

const BOOTAA64_PATH: &CStr16 = cstr16!(r"\EFI\BOOT\BOOTAA64.EFI");

// Global pointer to store the DisplayPowerProtocol
static DISPLAY_POWER_PROTOCOL: AtomicPtr<EfiDisplayPowerProtocol> = AtomicPtr::new(core::ptr::null_mut());

fn connect_all() -> Result {
    let handles = boot::locate_handle_buffer(SearchType::AllHandles)?;
    for handle in handles.iter() {
        let _ = boot::connect_controller(*handle, None, None, true);
    }

    Ok(())
}

fn load_bootaa64() -> Result<Option<Handle>> {
    let handles = boot::locate_handle_buffer(SearchType::ByProtocol(&SimpleFileSystem::GUID))?;
    for handle in handles.iter() {
        let device_path_protocol = boot::open_protocol_exclusive::<DevicePath>(*handle)?;

        let filesystem = boot::open_protocol_exclusive::<SimpleFileSystem>(*handle);
        let Ok(mut filesystem) = filesystem else {
            info!("Unable to open SimpleFileSystem protocol on handle");
            continue;
        };

        let volume = filesystem.open_volume();
        let Ok(mut volume) = volume else {
            info!("Unable to open volume");
            continue;
        };

        let bootaa64 = volume.open(BOOTAA64_PATH, FileMode::Read, FileAttribute::READ_ONLY);
        if bootaa64.is_ok() {
            let mut path_iterator = device_path_protocol.node_iter();

            let mut path_buf = [MaybeUninit::uninit(); 256];
            let path: &DevicePath = build::DevicePathBuilder::with_buf(&mut path_buf)
                .push(&path_iterator.next().unwrap())
                .unwrap()
                .push(&path_iterator.next().unwrap())
                .unwrap()
                .push(&build::media::FilePath {
                    path_name: BOOTAA64_PATH,
                })
                .unwrap()
                .finalize()
                .unwrap();

            let image = boot::load_image(
                boot::image_handle(),
                LoadImageSource::FromDevicePath {
                    device_path: path,
                    boot_policy: BootPolicy::ExactMatch,
                },
            )
            .expect("Failed to load image");

            return Ok(Some(image));
        }
    }

    Ok(None)
}

const READY_TO_BOOT: uefi::Guid = guid!("7ce88fb3-4bd7-4679-87a8-a8d8dee50d2b");
const END_OF_DXE: uefi::Guid = guid!("02ce967a-dd7e-4ffc-9ee7-810cf0470880");
const EFI_EVENT_DETECT_SD_CARD: uefi::Guid = guid!("b7972c36-8a4c-4a56-8b02-1159b52d4bfb");
const EFI_DISPLAY_POWER_PROTOCOL_GUID: uefi::Guid = guid!("f352021d-9593-4432-bf04-67b9f3b76008");

// Display Power State enum - renamed to follow Rust naming conventions
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
enum EfiDisplayPowerState {
    Unknown = 0,
    Off,    // Will be 1
    On,     // Will be 2
}

// Function pointer types for the protocol - renamed to follow Rust naming conventions
type EfiDisplayPowerSetDisplayPowerState = unsafe extern "efiapi" fn(
    this: *mut EfiDisplayPowerProtocol,
    power_state: EfiDisplayPowerState,
) -> uefi::Status;

type EfiDisplayPowerGetDisplayPowerState = unsafe extern "efiapi" fn(
    this: *mut EfiDisplayPowerProtocol,
    power_state: *mut EfiDisplayPowerState,
) -> uefi::Status;

// Display Power Protocol structure - renamed fields to follow Rust naming conventions
#[repr(C)]
struct EfiDisplayPowerProtocol {
    revision: u32,
    set_display_power_state: EfiDisplayPowerSetDisplayPowerState,
    get_display_power_state: EfiDisplayPowerGetDisplayPowerState,
}

fn initialize_display_protocol() -> Result {
    // Try to locate handles that support the DisplayPowerState protocol
    match boot::locate_handle_buffer(SearchType::ByProtocol(&EFI_DISPLAY_POWER_PROTOCOL_GUID)) {
        Ok(handles) => {
            if handles.is_empty() {
                info!("No DisplayPowerState protocol found");
                return Ok(());
            }

            info!("Found {} DisplayPowerState protocol handles", handles.len());

            // Try to work with the first handle
            let handle = handles[0];

            // Use raw UEFI calls similar to the C implementation
            let system_table_ptr = uefi::table::system_table_raw()
                .ok_or(uefi::Status::ABORTED)?;

            let boot_services = unsafe { (*system_table_ptr.as_ptr()).boot_services };

            if boot_services.is_null() {
                return Err(uefi::Status::ABORTED.into());
            }

            // Try HandleProtocol to get the interface
            let mut interface_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
            let status = unsafe {
                ((*boot_services).handle_protocol)(
                    handle.as_ptr(),
                    &EFI_DISPLAY_POWER_PROTOCOL_GUID,
                    &mut interface_ptr,
                )
            };

            if !status.is_success() {
                info!("Failed to get DisplayPowerState protocol interface: {:?}", status);
                return Ok(());
            }

            if interface_ptr.is_null() {
                info!("DisplayPowerState protocol interface is null");
                return Ok(());
            }

            // Store the protocol pointer globally
            let protocol_ptr = interface_ptr as *mut EfiDisplayPowerProtocol;
            DISPLAY_POWER_PROTOCOL.store(protocol_ptr, Ordering::SeqCst);

            // Test the protocol by getting the current state
            let mut current_state = EfiDisplayPowerState::Unknown;
            let get_status = unsafe {
                ((*protocol_ptr).get_display_power_state)(protocol_ptr, &mut current_state)
            };

            if get_status.is_success() {
                info!("Current display power state: {:?}", current_state as u32);

                // Test setting the display state to On to fix dead code warning
                info!("Testing setting display to ON state");
                let set_status = unsafe {
                    ((*protocol_ptr).set_display_power_state)(
                        protocol_ptr,
                        EfiDisplayPowerState::On
                    )
                };

                if set_status.is_success() {
                    info!("Successfully set display to ON state");
                } else {
                    info!("Failed to set display to ON state: {:?}", set_status);
                }
            } else {
                info!("Failed to get current display power state: {:?}", get_status);
            }

            info!("DisplayPowerProtocol initialized successfully");

            Ok(())
        }
        Err(e) => {
            info!("Failed to locate DisplayPowerState protocol: {:?}", e);
            Ok(())
        }
    }
}

unsafe extern "efiapi" fn disable_display_callback(
    _event: uefi::Event,
    _context: Option<NonNull<core::ffi::c_void>>,
) {
    // Use the already initialized protocol to turn off display
    let protocol_ptr = DISPLAY_POWER_PROTOCOL.load(Ordering::SeqCst);

    if !protocol_ptr.is_null() {
        // When called during ExitBootServices, we can't use info! logging
        let _ = ((*protocol_ptr).set_display_power_state)(
            protocol_ptr,
            EfiDisplayPowerState::Off
        );
    }
}

fn register_exit_boot_services_callback() -> Result {
    // Create an event for EXIT_BOOT_SERVICES using the proper EventType
    let _event = unsafe {
        boot::create_event(
            EventType::SIGNAL_EXIT_BOOT_SERVICES, 
            Tpl::CALLBACK,
            Some(disable_display_callback),
            None,
        )?
    };

    info!("ExitBootServices callback registered successfully");

    // Note: We don't close this event as it needs to remain active
    // Also, we don't need to store it in a static variable as the UEFI firmware
    // will maintain it until ExitBootServices is called

    Ok(())
}

fn signal_guid(guid: &uefi::Guid) -> Result {
    unsafe extern "efiapi" fn callback(_: uefi::Event, _: Option<NonNull<core::ffi::c_void>>) {}

    let guid: Option<NonNull<uefi::Guid>> = NonNull::new(&mut guid.clone());
    let event = unsafe {
        boot::create_event_ex(
            EventType::NOTIFY_SIGNAL,
            Tpl::NOTIFY,
            Some(callback),
            None,
            guid,
        )
    }
    .expect("Failed to create event");

    boot::signal_event(&event)?;
    boot::close_event(event)?;

    Ok(())
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    // Initialize DisplayPowerProtocol early
    if let Err(e) = initialize_display_protocol() {
        info!("Failed to initialize DisplayPowerProtocol: {:?}", e);
    }

    // Register ExitBootServices callback using the correct event type
    if let Err(e) = register_exit_boot_services_callback() {
        info!("Failed to register ExitBootServices callback: {:?}", e);
    }

    signal_guid(&EFI_EVENT_DETECT_SD_CARD).expect("Failed to signal SD-card detect");

    connect_all().expect("Failed to connect drivers");

    signal_guid(&READY_TO_BOOT).expect("Failed to signal ReadyToBoot");

    signal_guid(&END_OF_DXE).expect("Failed to signal end of DXE");

    let image = load_bootaa64()
        .expect("An error occurred while searching for bootaa64.efi")
        .expect("No bootaa64.efi found");

    info!("Found bootaa64.efi, starting..");
    boot::start_image(image).expect("Failed to start bootaa64.efi");

    boot::stall(10_000_000);

    Status::NOT_FOUND
}
