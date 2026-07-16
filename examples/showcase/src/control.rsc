<input disabled={self.disabled}
       placeholder={self.placeholder}
       class=[self.extra, "base", { "invalid": self.invalid }]
       class:compact={self.compact}
       class:base={!self.invalid}
       {...self.wiring}
       {...&self.data}/>
