#pragma once

#include <EvepassCoreSpecJSI.h>

#include <memory>

namespace facebook::react {

class EvepassCoreImpl
  : public NativeEvepassCoreCxxSpec<EvepassCoreImpl> {
public:
  EvepassCoreImpl(std::shared_ptr<CallInvoker> jsInvoker);

  double multiply(jsi::Runtime& rt, double a, double b);
};

}
