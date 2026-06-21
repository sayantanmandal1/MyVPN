import { Navbar } from "@/components/site/navbar";
import { Hero } from "@/components/site/hero";
import { Features } from "@/components/site/features";
import { HowItWorks } from "@/components/site/how-it-works";
import { Security } from "@/components/site/security";
import { DownloadCta } from "@/components/site/download-cta";
import { Footer } from "@/components/site/footer";

export default function Home() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <Features />
        <HowItWorks />
        <Security />
        <DownloadCta />
      </main>
      <Footer />
    </>
  );
}
