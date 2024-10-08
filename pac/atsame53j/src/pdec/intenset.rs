#[doc = "Register `INTENSET` reader"]
pub type R = crate::R<IntensetSpec>;
#[doc = "Register `INTENSET` writer"]
pub type W = crate::W<IntensetSpec>;
#[doc = "Field `OVF` reader - Overflow/Underflow Interrupt Enable"]
pub type OvfR = crate::BitReader;
#[doc = "Field `OVF` writer - Overflow/Underflow Interrupt Enable"]
pub type OvfW<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `ERR` reader - Error Interrupt Enable"]
pub type ErrR = crate::BitReader;
#[doc = "Field `ERR` writer - Error Interrupt Enable"]
pub type ErrW<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `DIR` reader - Direction Interrupt Enable"]
pub type DirR = crate::BitReader;
#[doc = "Field `DIR` writer - Direction Interrupt Enable"]
pub type DirW<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `VLC` reader - Velocity Interrupt Enable"]
pub type VlcR = crate::BitReader;
#[doc = "Field `VLC` writer - Velocity Interrupt Enable"]
pub type VlcW<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `MC0` reader - Channel 0 Compare Match Enable"]
pub type Mc0R = crate::BitReader;
#[doc = "Field `MC0` writer - Channel 0 Compare Match Enable"]
pub type Mc0W<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `MC1` reader - Channel 1 Compare Match Enable"]
pub type Mc1R = crate::BitReader;
#[doc = "Field `MC1` writer - Channel 1 Compare Match Enable"]
pub type Mc1W<'a, REG> = crate::BitWriter<'a, REG>;
impl R {
    #[doc = "Bit 0 - Overflow/Underflow Interrupt Enable"]
    #[inline(always)]
    pub fn ovf(&self) -> OvfR {
        OvfR::new((self.bits & 1) != 0)
    }
    #[doc = "Bit 1 - Error Interrupt Enable"]
    #[inline(always)]
    pub fn err(&self) -> ErrR {
        ErrR::new(((self.bits >> 1) & 1) != 0)
    }
    #[doc = "Bit 2 - Direction Interrupt Enable"]
    #[inline(always)]
    pub fn dir(&self) -> DirR {
        DirR::new(((self.bits >> 2) & 1) != 0)
    }
    #[doc = "Bit 3 - Velocity Interrupt Enable"]
    #[inline(always)]
    pub fn vlc(&self) -> VlcR {
        VlcR::new(((self.bits >> 3) & 1) != 0)
    }
    #[doc = "Bit 4 - Channel 0 Compare Match Enable"]
    #[inline(always)]
    pub fn mc0(&self) -> Mc0R {
        Mc0R::new(((self.bits >> 4) & 1) != 0)
    }
    #[doc = "Bit 5 - Channel 1 Compare Match Enable"]
    #[inline(always)]
    pub fn mc1(&self) -> Mc1R {
        Mc1R::new(((self.bits >> 5) & 1) != 0)
    }
}
impl W {
    #[doc = "Bit 0 - Overflow/Underflow Interrupt Enable"]
    #[inline(always)]
    #[must_use]
    pub fn ovf(&mut self) -> OvfW<IntensetSpec> {
        OvfW::new(self, 0)
    }
    #[doc = "Bit 1 - Error Interrupt Enable"]
    #[inline(always)]
    #[must_use]
    pub fn err(&mut self) -> ErrW<IntensetSpec> {
        ErrW::new(self, 1)
    }
    #[doc = "Bit 2 - Direction Interrupt Enable"]
    #[inline(always)]
    #[must_use]
    pub fn dir(&mut self) -> DirW<IntensetSpec> {
        DirW::new(self, 2)
    }
    #[doc = "Bit 3 - Velocity Interrupt Enable"]
    #[inline(always)]
    #[must_use]
    pub fn vlc(&mut self) -> VlcW<IntensetSpec> {
        VlcW::new(self, 3)
    }
    #[doc = "Bit 4 - Channel 0 Compare Match Enable"]
    #[inline(always)]
    #[must_use]
    pub fn mc0(&mut self) -> Mc0W<IntensetSpec> {
        Mc0W::new(self, 4)
    }
    #[doc = "Bit 5 - Channel 1 Compare Match Enable"]
    #[inline(always)]
    #[must_use]
    pub fn mc1(&mut self) -> Mc1W<IntensetSpec> {
        Mc1W::new(self, 5)
    }
}
#[doc = "Interrupt Enable Set\n\nYou can [`read`](crate::Reg::read) this register and get [`intenset::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`intenset::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct IntensetSpec;
impl crate::RegisterSpec for IntensetSpec {
    type Ux = u8;
}
#[doc = "`read()` method returns [`intenset::R`](R) reader structure"]
impl crate::Readable for IntensetSpec {}
#[doc = "`write(|w| ..)` method takes [`intenset::W`](W) writer structure"]
impl crate::Writable for IntensetSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u8 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u8 = 0;
}
#[doc = "`reset()` method sets INTENSET to value 0"]
impl crate::Resettable for IntensetSpec {
    const RESET_VALUE: u8 = 0;
}
