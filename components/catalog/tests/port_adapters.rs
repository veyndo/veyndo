//! Test adapters for the catalog ports (`Shoppers`, `Shop`, `Images`), so the catalog mirror builds and tests without the server.
use sqlx::PgConnection;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_catalog::ports::{
    ContentLanguages, ImageProvider, Images, Shop, Shopper, Shoppers, Visit,
};
use veyndo_core::{PortFuture, RequestContext, Result, bad};

/// Polls a test adapter's answer; adapters answer at once, without a runtime.
fn ready<T>(mut future: PortFuture<'_, T>) -> Result<T> {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a test adapter answers at once"),
    }
}

/// An anonymous shopper whose cart bought exactly one product.
struct OneBuyer;

impl Shoppers for OneBuyer {
    fn current<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, Shopper> {
        Box::pin(async {
            Ok(Shopper {
                tenant: "shop-a".into(),
                cart: "cart-1".into(),
                customer: None,
            })
        })
    }
    fn bought<'a>(&'a self, shopper: &'a Shopper, product: &'a str) -> PortFuture<'a, bool> {
        let bought = shopper.cart == "cart-1" && product == "chair";
        Box::pin(async move { Ok(bought) })
    }
}

/// A live shop in German and English that offers every product.
struct LiveShop;

impl LiveShop {
    fn languages() -> ContentLanguages {
        ContentLanguages {
            main: "de-DE".into(),
            enabled: vec!["de-DE".into(), "en-GB".into()],
        }
    }
}

impl Shop for LiveShop {
    fn languages<'a>(&'a self, _: &'a str) -> PortFuture<'a, ContentLanguages> {
        Box::pin(async { Ok(Self::languages()) })
    }
    fn languages_shared<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
    ) -> PortFuture<'a, ContentLanguages> {
        Box::pin(async { Ok(Self::languages()) })
    }
    fn locales<'a>(&'a self, languages: &'a [String]) -> PortFuture<'a, Vec<String>> {
        let known = Self::languages().enabled;
        let locales = languages
            .iter()
            .filter(|l| known.contains(l))
            .cloned()
            .collect();
        Box::pin(async move { Ok(locales) })
    }
    fn visit<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, Visit> {
        Box::pin(async {
            Ok(Visit {
                tenant: "shop-a".into(),
                locale: "de-DE".into(),
                navigation_root: "root".into(),
            })
        })
    }
    fn admit_product<'a>(&'a self, _: &'a RequestContext, _: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn is_preview<'a>(&'a self, _: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn attribute<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a RequestContext,
        _: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

/// No image model is configured; creating an image is refused, never attempted.
struct NoImages;

impl Images for NoImages {
    fn provider<'a>(&'a self) -> PortFuture<'a, ImageProvider> {
        Box::pin(async {
            Ok(ImageProvider {
                configured: false,
                model: String::new(),
            })
        })
    }
    fn create<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: Option<(String, Vec<u8>)>,
    ) -> PortFuture<'a, Vec<u8>> {
        Box::pin(async { Err(bad("no image model configured")) })
    }
}

#[test]
fn catalog_ports_answer_without_the_server() {
    let request = RequestContext::new();
    let shoppers: Arc<dyn Shoppers> = Arc::new(OneBuyer);
    let shopper = ready(shoppers.current(&request)).unwrap();
    assert!(ready(shoppers.bought(&shopper, "chair")).unwrap());
    assert!(!ready(shoppers.bought(&shopper, "table")).unwrap());
    let shop: Arc<dyn Shop> = Arc::new(LiveShop);
    let languages = ready(shop.languages("shop-a")).unwrap();
    assert_eq!(languages.as_ref().main, "de-DE");
    let wanted = vec!["en-GB".to_string(), "fr-FR".to_string()];
    assert_eq!(
        ready(shop.locales(&wanted)).unwrap(),
        vec!["en-GB".to_string()]
    );
    assert_eq!(ready(shop.visit(&request)).unwrap().navigation_root, "root");
    let images: Arc<dyn Images> = Arc::new(NoImages);
    assert!(!ready(images.provider()).unwrap().configured);
    assert!(ready(images.create("shop-a", "a chair", None)).is_err());
}
