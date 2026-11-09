//! Desktop Windows Hello through the HWND-aware WinRT interop API.

#![allow(unsafe_code)]

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
use windows::Win32::Foundation::{E_NOINTERFACE, HWND, REGDB_E_CLASSNOTREG};
use windows::Win32::System::WinRT::{IUserConsentVerifierInterop, RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::{HSTRING, factory};
use windows_future::IAsyncOperation;

use crate::{Unlock, UnlockError};

#[derive(Debug, Default)]
pub struct WindowsHello {
    parent: AtomicIsize,
}

impl Unlock for WindowsHello {
    fn set_parent_window(&self, window: isize) {
        self.parent.store(window, Ordering::Release);
    }

    fn unlock(&self, reason: &str) -> Result<(), UnlockError> {
        let parent = self.parent.load(Ordering::Acquire);
        if parent == 0 {
            return Err(UnlockError::Cancelled);
        }
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| UnlockError::Cancelled)?;
        let _apartment = Apartment;
        let consent: IUserConsentVerifierInterop = factory::<UserConsentVerifier, _>().map_err(factory_error)?;
        let operation: IAsyncOperation<UserConsentVerificationResult> =
            unsafe { consent.RequestVerificationForWindowAsync(HWND(parent as *mut _), &HSTRING::from(reason)) }
                .map_err(|_| UnlockError::Cancelled)?;
        let result = operation.join().map_err(|_| UnlockError::Cancelled)?;
        outcome(result)
    }
}

fn factory_error(error: windows::core::Error) -> UnlockError {
    match error.code() {
        E_NOINTERFACE | REGDB_E_CLASSNOTREG => UnlockError::Unavailable,
        _ => UnlockError::Cancelled,
    }
}

fn outcome(result: UserConsentVerificationResult) -> Result<(), UnlockError> {
    match result {
        UserConsentVerificationResult::Verified => Ok(()),
        UserConsentVerificationResult::DeviceNotPresent
        | UserConsentVerificationResult::NotConfiguredForUser
        | UserConsentVerificationResult::DisabledByPolicy => Err(UnlockError::Unavailable),
        _ => Err(UnlockError::Cancelled),
    }
}

struct Apartment;

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_verified_consent_unlocks() {
        assert_eq!(outcome(UserConsentVerificationResult::Verified), Ok(()));
        for result in [
            UserConsentVerificationResult::Canceled,
            UserConsentVerificationResult::DeviceBusy,
            UserConsentVerificationResult::RetriesExhausted,
            UserConsentVerificationResult(99),
        ] {
            assert_eq!(outcome(result), Err(UnlockError::Cancelled));
        }
        assert_eq!(outcome(UserConsentVerificationResult::NotConfiguredForUser), Err(UnlockError::Unavailable));
    }

    #[test]
    fn an_absent_os_api_is_distinct_from_a_failed_consent_request() {
        assert_eq!(factory_error(windows::core::Error::from_hresult(E_NOINTERFACE)), UnlockError::Unavailable);
        assert_eq!(factory_error(windows::core::Error::from_hresult(REGDB_E_CLASSNOTREG)), UnlockError::Unavailable);
        assert_eq!(
            factory_error(windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))),
            UnlockError::Cancelled
        );
    }
}
