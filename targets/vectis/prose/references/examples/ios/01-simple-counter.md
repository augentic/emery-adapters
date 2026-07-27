# Example: Simple Counter iOS Shell (Render Only)

A minimal iOS shell for a Crux counter app with local state and no external side-effects. Demonstrates Core.swift, ContentView, and screen views. Makefile / `project.yml` authority is `$TEMPLATE_DIR` — not this example.

This shell pairs with the core-writer example `01-simple-counter.md`. The shared crate defines:

- `ViewModel::Loading` and `ViewModel::Counter(CounterView)` variants
- `Event::Navigate(Route)`, `Event::Increment`, `Event::Decrement`, `Event::Reset`
- `Effect::Render(RenderOperation)`
- `Route::Counter`
- `CounterView { count: String }`

## Capabilities Handled

- **Render** -- update the published `ViewModel`

## Directory Structure

```
examples/counter/
    shared/             # Already exists from core-writer
    iOS/
        project.yml
        Makefile
        Counter/
            CounterApp.swift
            Core.swift
            ContentView.swift
            Views/
                LoadingScreen.swift
                CounterScreen.swift
            Theme/                # generated from design-system/tokens.yaml
                Colors.swift
                Typography.swift
                Spacing.swift
                Theme.swift
```

## `iOS/Makefile` and `iOS/project.yml`

Do **not** hand-copy DX from this example. Materialize (or re-copy) `iOS/Makefile` and `iOS/project.yml` from `$TEMPLATE_DIR` with identity substitution. The live template owns BoltFFI pack (`boltffi pack apple`), `DESTINATION ?= generic/platform=iOS Simulator`, and the `./generated/Shared` package path. There are no `iOS/.vectis/sim-*.sh` scripts and no `cargo-swift` / `sharedFFI` recipes.

## Local run

```bash
cd iOS && make build && make run-sim
```

Built artifact: `iOS/DerivedData/Build/Products/Debug-iphonesimulator/Counter-iOS.app`. Override simulator with `SIMULATOR_UDID` or `iOS/.env.local`.

## `iOS/Counter/CounterApp.swift`

```swift
import Inject
import SwiftUI

@main
struct CounterApp: App {
    @StateObject private var core = Core()
    @ObserveInjection var inject

    var body: some Scene {
        WindowGroup {
            ContentView(core: core)
                .vectisTheme()
        }
    }
}
```

## `iOS/Counter/Core.swift`

```swift
import Foundation
import Shared
import SharedTypes

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel

    private let core: CoreFfi

    init() {
        self.core = CoreFfi()
        do {
            self.view = Self.deserializeView(try core.view())
        } catch {
            assertionFailure("Failed to get initial view from core: \(error)")
            self.view = .loading
        }
    }

    func update(_ event: Event) {
        guard let data = try? event.bincodeSerialize() else {
            assertionFailure("Failed to serialize event: \(event)")
            return
        }
        do {
            let effects = try core.update(data: Data(data))
            processEffects([UInt8](effects))
        } catch {
            assertionFailure("Failed to update core: \(error)")
        }
    }

    private func processEffects(_ data: [UInt8]) {
        guard let requests = try? [Request].bincodeDeserialize(input: data) else {
            assertionFailure("Failed to deserialize requests")
            return
        }
        for request in requests {
            processEffect(request)
        }
    }

    func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            do {
                let data = try core.view()
                guard let vm = try? ViewModel.bincodeDeserialize(input: [UInt8](data)) else {
                    assertionFailure("Failed to deserialize ViewModel from bincode")
                    break
                }
                self.view = vm
            } catch {
                assertionFailure("Failed to get view from core: \(error)")
            }
        }
    }

    /// Only used during `init()` where `.loading` is the correct fallback.
    /// The `.render` handler preserves the existing view on failure.
    private static func deserializeView(_ data: Data) -> ViewModel {
        guard let vm = try? ViewModel.bincodeDeserialize(input: [UInt8](data)) else {
            assertionFailure("Failed to deserialize ViewModel from bincode")
            return .loading
        }
        return vm
    }
}
```

## `iOS/Counter/ContentView.swift`

```swift
import Inject
import SwiftUI

struct ContentView: View {
    @ObservedObject var core: Core
    @ObserveInjection var inject

    var body: some View {
        switch core.view {
        case .loading:
            LoadingScreen()
        case .counter(let viewModel):
            CounterScreen(viewModel: viewModel) { event in
                core.update(event)
            }
        }
        .enableInjection()
    }
}
```

## `iOS/Counter/Views/LoadingScreen.swift`

```swift
import Inject
import SwiftUI

struct LoadingScreen: View {
    @ObserveInjection var inject

    var body: some View {
        VStack(spacing: VectisSpacing.md) {
            ProgressView()
                .controlSize(.large)
            Text("Loading...")
                .font(VectisTypography.body)
                .foregroundStyle(VectisColors.onSurfaceSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(VectisColors.surface)
        .enableInjection()
    }
}

#Preview {
    LoadingScreen()
        .vectisTheme()
}
```

## `iOS/Counter/Views/CounterScreen.swift`

```swift
import Inject
import SwiftUI

struct CounterScreen: View {
    let viewModel: CounterView
    let onEvent: (Event) -> Void
    @ObserveInjection var inject

    var body: some View {
        VStack(spacing: VectisSpacing.lg) {
            Spacer()

            Text(viewModel.count)
                .font(VectisTypography.largeTitle)
                .foregroundStyle(VectisColors.onSurface)

            HStack(spacing: VectisSpacing.md) {
                Button {
                    onEvent(.decrement)
                } label: {
                    Image(systemName: "minus.circle.fill")
                        .font(.system(size: 44))
                }
                .tint(VectisColors.secondary)
                .accessibilityLabel("Decrement")

                Button {
                    onEvent(.reset)
                } label: {
                    Image(systemName: "arrow.counterclockwise.circle.fill")
                        .font(.system(size: 44))
                }
                .tint(VectisColors.error)
                .accessibilityLabel("Reset")

                Button {
                    onEvent(.increment)
                } label: {
                    Image(systemName: "plus.circle.fill")
                        .font(.system(size: 44))
                }
                .tint(VectisColors.primary)
                .accessibilityLabel("Increment")
            }

            Spacer()
        }
        .frame(maxWidth: .infinity)
        .background(VectisColors.surface)
        .enableInjection()
    }
}

#Preview {
    CounterScreen(
        viewModel: CounterView(count: "Count is: 42"),
        onEvent: { _ in }
    )
    .vectisTheme()
}
```

## Key Patterns Demonstrated

1. **One screen per ViewModel variant** -- `LoadingScreen` and `CounterScreen`.
2. **Event callback pattern** -- screens receive `(Event) -> Void`, not the `Core`.
3. **Shell-local theme tokens** -- all colors, fonts, and spacing resolve to the shell-local `Theme/` enums (`VectisColors`, `VectisTypography`, `VectisSpacing`) generated from `tokens.yaml`. There is no external Swift Package and no `import VectisDesign`.
4. **Preview support** -- every screen has a `#Preview` with sample data.
5. **Accessibility** -- interactive icons have `accessibilityLabel`.
6. **Render-only Core.swift** -- the simplest possible effect handler.
7. **Hot reloading** -- Inject boilerplate (`@ObserveInjection`, `.enableInjection()`) in every view; Debug-only linker flags in `project.yml`.
