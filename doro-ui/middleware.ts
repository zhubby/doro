import { NextResponse, type NextRequest } from "next/server";

import { defaultLocale, isAppLocale, locales } from "@/i18n/routing";

function pathnameLocale(pathname: string) {
  const firstSegment = pathname.split("/")[1];
  return isAppLocale(firstSegment) ? firstSegment : null;
}

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const locale = pathnameLocale(pathname);

  if (locale) {
    const response = NextResponse.next();
    response.cookies.set("doro-locale", locale, {
      path: "/",
      sameSite: "lax",
    });
    return response;
  }

  const cookieLocale = request.cookies.get("doro-locale")?.value;
  const preferredLocale = isAppLocale(cookieLocale ?? "")
    ? (cookieLocale as (typeof locales)[number])
    : defaultLocale;
  const url = request.nextUrl.clone();
  url.pathname = `/${preferredLocale}${pathname === "/" ? "" : pathname}`;

  const response = NextResponse.rewrite(url);
  response.cookies.set("doro-locale", preferredLocale, {
    path: "/",
    sameSite: "lax",
  });
  return response;
}

export const config = {
  matcher: ["/((?!api|_next|.*\\..*).*)"],
};
