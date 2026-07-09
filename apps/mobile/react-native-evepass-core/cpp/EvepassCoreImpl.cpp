#include "EvepassCoreImpl.h"
#include "react-native-evepass-core.h"

namespace facebook::react {

EvepassCoreImpl::EvepassCoreImpl(
  std::shared_ptr<CallInvoker> jsInvoker
)
  : NativeEvepassCoreCxxSpec(std::move(jsInvoker)) {}

bool EvepassCoreImpl::installRustCrate(jsi::Runtime& rt) {
  return evepasscore::installRustCrate(rt, jsInvoker_);
}

bool EvepassCoreImpl::cleanupRustCrate(jsi::Runtime& rt) {
  return evepasscore::cleanupRustCrate(rt);
}

}
