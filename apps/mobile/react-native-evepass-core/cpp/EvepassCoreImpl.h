#pragma once

#include <EvepassCoreSpecJSI.h>

#include <memory>

namespace facebook::react {

// TurboModule that installs the UniFFI-generated evepass-core bindings into the
// JSI runtime. The two methods below match the UBRN-generated spec
// (src/NativeEvepassCore.ts) and delegate to the generated installer in
// cpp/react-native-evepass-core.cpp (namespace `evepasscore`).
class EvepassCoreImpl
  : public NativeEvepassCoreCxxSpec<EvepassCoreImpl> {
public:
  EvepassCoreImpl(std::shared_ptr<CallInvoker> jsInvoker);

  bool installRustCrate(jsi::Runtime& rt);
  bool cleanupRustCrate(jsi::Runtime& rt);
};

}
