#import <Foundation/Foundation.h>
#import "EvepassCoreImpl.h"
#import <ReactCommon/CxxTurboModuleUtils.h>

@interface EvepassCoreOnLoad : NSObject
@end

@implementation EvepassCoreOnLoad

using namespace facebook::react;

+ (void)load
{
  registerCxxModuleToGlobalModuleMap(
    std::string(EvepassCoreImpl::kModuleName),
    [](std::shared_ptr<CallInvoker> jsInvoker) {
      return std::make_shared<EvepassCoreImpl>(jsInvoker);
    }
  );
}

@end
