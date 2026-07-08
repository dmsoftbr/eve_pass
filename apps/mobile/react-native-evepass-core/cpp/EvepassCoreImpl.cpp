#include "EvepassCoreImpl.h"

namespace facebook::react {

EvepassCoreImpl::EvepassCoreImpl(
  std::shared_ptr<CallInvoker> jsInvoker
)
  : NativeEvepassCoreCxxSpec(std::move(jsInvoker)) {}

double EvepassCoreImpl::multiply(
  jsi::Runtime& rt,
  double a,
  double b
) {
  return a * b;
}

}
