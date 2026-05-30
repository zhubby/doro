import { Suspense } from "react";

import { LoginPage } from "@/components/login/login-page";

export default function LoginRoute() {
  return (
    <Suspense>
      <LoginPage />
    </Suspense>
  );
}
