// UniFFI normalizes consecutive capitals (Rust `DHService` generates Swift
// `DhService`); these aliases restore the intended DH prefix for app code.
typealias DHService = DhService
typealias DHEvent = DhEvent
typealias DHError = DhError
