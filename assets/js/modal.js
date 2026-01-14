/*
 * Modal
 *
 * Pico.css - https://picocss.com
 * Copyright 2019-2024 - Licensed under MIT
 */

// Config
const isOpenClass = "modal-is-open";
const openingClass = "modal-is-opening";
const closingClass = "modal-is-closing";
const scrollbarWidthCssVar = "--pico-scrollbar-width";
const animationDuration = 400; // ms
let visibleModal = null;

// Toggle modal
const toggleModal = (event) => {
  event.preventDefault();
  const modal = document.getElementById(event.currentTarget.dataset.target);
  if (!modal) return;
  modal && (modal.open ? closeModal(modal) : openModal(modal));
};

// Open modal
const openModal = (modal) => {
  const { documentElement: html } = document;
  const scrollbarWidth = getScrollbarWidth();
  if (scrollbarWidth) {
    html.style.setProperty(scrollbarWidthCssVar, `${scrollbarWidth}px`);
  }
  html.classList.add(isOpenClass, openingClass);
  setTimeout(() => {
    visibleModal = modal;
    html.classList.remove(openingClass);
  }, animationDuration);
  modal.showModal();
};

// Close modal
const closeModal = (modal) => {
  visibleModal = null;
  const { documentElement: html } = document;
  html.classList.add(closingClass);
  setTimeout(() => {
    html.classList.remove(closingClass, isOpenClass);
    html.style.removeProperty(scrollbarWidthCssVar);
    modal.close();
  }, animationDuration);
};

// Close with a click outside
document.addEventListener("click", (event) => {
  if (visibleModal === null) return;
  const modalContent = visibleModal.querySelector("article");
  const isClickInside = modalContent.contains(event.target);
  !isClickInside && closeModal(visibleModal);
});

// Close with Esc key
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && visibleModal) {
    closeModal(visibleModal);
  }
});

// Get scrollbar width
const getScrollbarWidth = () => {
  const scrollbarWidth = window.innerWidth - document.documentElement.clientWidth;
  return scrollbarWidth;
};

// Is scrollbar visible
const isScrollbarVisible = () => {
  return document.body.scrollHeight > screen.height;
};

/* --- Signup Logic Integration --- */

async function asyncSubmitSignup(event) {
  event.preventDefault();
  
  // 1. Get Elements
  const form = event.currentTarget;
  const emailInput = form.querySelector('input[name="email"]');
  const submitBtn = form.querySelector('button[type="submit"]');
  
  const modal = document.getElementById("modal-example");
  const modalTitle = modal.querySelector("h3");
  const modalBody = modal.querySelector("p");
  const modalFooter = modal.querySelector("footer");

  
  // 2. UI Loading State (Pico's loading spinner)
  submitBtn.setAttribute("aria-busy", "true");
  submitBtn.disabled = true;

  try {
    const response = await fetch('/signup', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email: emailInput.value })
    });

    const resultText = await response.text();

    submitBtn.setAttribute("aria-busy", "false");

    // 3. Open Modal with Result
    if (response.ok) {
      modalTitle.innerText = "Success!";
      modalBody.innerHTML = "Check your email for a verification link.<br>";

      // Reset form
      form.reset();
      // Ensure button is disabled (form reset unchecks the box)
      submitBtn.disabled = true; 

      // Hide the "Confirm" button in the footer since we are done
      if(modalFooter.querySelector('button:not(.secondary)')) {
          modalFooter.querySelector('button:not(.secondary)').style.display = 'none';
      }
    } else {
      modalTitle.innerText = "Signup Failed";
      modalBody.innerText = resultText || "Something went wrong. Please try again.";
      // Re-enable button on failure so they can try again
      submitBtn.disabled = false;
    }
  } catch (err) {
    submitBtn.setAttribute("aria-busy", "false");
    submitBtn.disabled = false;
    modalTitle.innerText = "Connection Error";
    modalBody.innerText = "Something went wrong. Please try again";
  }
  
  openModal(modal);
}

// Global wrapper to match your onclick
window.submitSignup = asyncSubmitSignup;
