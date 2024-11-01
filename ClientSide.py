import socket
import json
import time
import random
import threading
import customtkinter as ctk
from datetime import datetime
import ssl
import struct

# Declare config settings for the server
SERVER_HOST = "5.75.133.232"
SERVER_PORT = 8080


class SmartMeterGUI(ctk.CTk):
    # SmartMeterGUI() Class:
    # Handles functionality related to the client side, draws relevant customtkinter GUI elements & updates them. 
    # Also handles behaviours such as sending reading messages to the server and processing bill responses
    # from the server.
    
    def __init__(self, id):
        # SmartMeterGUI() Constructor:
        # Creates window with all relevant customtkinter GUI elements, each elemeent is given an 'initial value'
        # that is held until the first bill response is recieved from the server.

        super().__init__()
        # SmartMeter  properties:id, title, size, bg colour.
        self.id = id
        self.title("Smart Meter")
        self.geometry("480x250")
        self.configure(bg="black")

        # Date and time display : Shows current date/time.
        self.time_label = ctk.CTkLabel(self, text="", font=("Arial", 12), text_color="white")
        self.time_label.place(x=250, y=10)  

        # Electricity Icon : Image on GUI.
        self.electric_icon_label = ctk.CTkLabel(self, text="⚡", font=("Arial", 40), text_color="yellow")
        self.electric_icon_label.place(x=20, y=60)

        # Cost display : Running count of total cost from bills (Unit : £).
        self.cost_label = ctk.CTkLabel(self, text="£0.00", font=("Arial", 24), text_color="white")
        self.cost_label.place(x=100, y=60)

        # Units Used display : Running count of total no of kWh's used (Unit : kWh)
        self.units_used_label = ctk.CTkLabel(self, text="Units Used: 0.0 kWh", font=("Arial", 12), text_color="white")
        self.units_used_label.place(x=100, y=90)

        # price per unit display : Displays the cost of each kWh  (Unit : £)
        self.price_per_unit_label = ctk.CTkLabel(self, text="Price per kWh: £0.00", font=("Arial", 12), text_color="white")
        self.price_per_unit_label.place(x=100, y=120)

        # Standing Charge display : Displays the total costs of standing charge (Unit : kWh)
        self.standing_charge_label = ctk.CTkLabel(self, text="Standing Charge: £0.00", font=("Arial", 12), text_color="white")
        self.standing_charge_label.place(x=300, y=90) 

        # billing period display : Displays the length of the billing period (Unit : Days)
        self.billing_period_label = ctk.CTkLabel(self, text="Billing Period: 0 Days", font=("Arial", 12), text_color="white")
        self.billing_period_label.place(x=300, y=120)

        # Status of server connection display : Displays the connection status between this client and the server, also displays if there is a problem with the power grid.
        self.status_label = ctk.CTkLabel(self, text="Not Connected", font=("Arial", 12), text_color="white")
        self.status_label.place(x=10, y=180)

        self.last_update_label = ctk.CTkLabel(self, text="Last Update", font=("Arial", 12), text_color="white")
        self.last_update_label.place(x=300, y=180)

        # Exit button to close the application : Closes this client.
        self.exit_button = ctk.CTkButton(self, text="Exit", command=self.exit)
        self.exit_button.place(x=170, y=210)

        # Start updating time
        self.update_time()

        # Start the client automatically
        threading.Thread(target=self.auto_connect, daemon=True).start()

        # Declare attributes for readings & billing info from server & assign default values.
        self.cumulative_reading = 0.0
        self.total_bill = 0.0
        self.units_used = 0.0
        self.standing_charge = 0.0
        self.price_per_unit = 0.0
        self.billing_period = 0

        self.started = None

    def update_time(self):
        # update_time() Method:
        # Updates current time display on GUI.
        self.time_label.configure(text=datetime.now().strftime("%A, %B %d, %Y %H:%M:%S"))
        self.after(1000, self.update_time)


    def auto_connect(self):
        # auto_connect() Method:
        # Updates status label and tries to connect the client to the server.
        self.status_label.configure(text="Connecting...")
        run_client(self, self.id)


    def exit(self):
        # exit() Method:
        # Closes the current SmartMeter client.
        self.destroy()


    def update_GUI(self, total_bill, standing_charge, price_per_unit, units_used, billing_period):
        # update_GUI() Method:
        # Receives billing info as paramaters and updates related GUI labels.

        # Update labels (column 1 : Energy info)
        self.cost_label.configure(text=f"£{total_bill:.2f}")
        self.units_used_label.configure(text=f"Units Used: {units_used:.2f} kWh")
        self.price_per_unit_label.configure(text=f"Price per kWh: £{price_per_unit:.2f}")
        # Update labels (column 2 : Standing charge & billing period info)
        self.standing_charge_label.configure(text=f"Standing Charge: £{standing_charge:.2f}")
        self.billing_period_label.configure(text=f"Billing Period: {billing_period}")


    def set_status(self, message):
        # set_status() Method:
        # Updates the status of the connection between the client and the server.
        self.status_label.configure(text=message)


    def trigger_reading_event(self):
        # Generate a new reading
        new_reading = generate_meter_reading()

        # Update cumulative reading
        self.cumulative_reading += new_reading

        # Create the reading data with cumulative reading
        reading_data = {
            "type": "MeterReading",
            "reading": self.cumulative_reading,  # Send cumulative reading
        }
        threading.Thread(target=self.response_from_server, args=(reading_data,)).start()  # Send the reading in a new thread

        


    def response_from_server(self, reading_data):
        # response_from_server() Method :
        # Sends the reading to the server and checks to see if server responds, if response received, update GUI elements.

        # sends reading to server using reading_data, returns server response.
        response = send_reading_to_server(self.sock, reading_data)

        # Handle response received from server.
        if response:
            print(f"Server response: {response}")
            try:
                # Decode the message from the server
                message_data = json.loads(response)
                self.handle_server_message(message_data)
            except (json.JSONDecodeError, ValueError) as e:
                # If the response isn't able to be decoded from object to string, output error & set status as 'Error'.
                print(f"Error parsing server response: {e}")
                self.set_status("Error")
        else:
            # No response from server, set status as 'Disconnected'.
            print("Disconnected from server, will retry")
            # Try to reconnect.
            run_client(self, id)

    def handle_server_message(self, message_data):
        # handle_server_message() Method:
        # Handles received JSON message & unpacks data into variables for the smart meter. Calls method to update  
        # the SmartMeterGUI elements.
        try:

            # Handle bill data.
            if message_data.get("type") == "Bill":
                # Handle bill data
                bill_info = message_data

                # Unpack bill data safely using .get() to avoid missing key issues
                standing_charge = bill_info.get('standing_charge', 0.0)
                total = bill_info.get('total', 0.0)  # This is the new total usage cost
                units_used = bill_info.get('units_used', 0.0)
                price_per_unit = bill_info.get('price_per_unit', 0.0)
                daily_standing_charge = bill_info.get('daily_standing_charge', 0.0)
                billing_period = bill_info.get('billing_period', {})
                billing_start = billing_period.get("start", " ")  
                billing_end = billing_period.get("end", " ")  

                # Combine start and end into a single string
                billing_period_str = f"{billing_start} - {billing_end}"

                # Update the cumulative total bill
                self.total_bill = total

                # Update other attributes for GUI display
                self.standing_charge = standing_charge
                self.daily_standing_charge = daily_standing_charge
                self.price_per_unit = price_per_unit
                self.billing_period = billing_period_str  
                self.units_used = units_used

                # Update the GUI with the new data
                self.after(1, self.update_GUI,
                        self.total_bill,
                        self.standing_charge, 
                        self.price_per_unit,
                        self.units_used, 
                        self.billing_period)

            # Handle power grid issue data.
            elif message_data.get("type") == "PowerGridDown":
                # Unpack power grid issue data safely using .get() to avoid missing key issues.
                error_info = message_data
                print(error_info)
                error_message = error_info.get('error', 'Unknown error')
                
                # Update on GUI and print message.
                print(f"Power Grid Issue: {error_message}")
                self.set_status(f"Power Grid Issue: {error_message}")

            else:
                # If the message type is not recognized, print a warning message.
                print("Unknown message type received:", message_data)
        except Exception as e:
            print(e)

    def updatetime(self, started):
    # Start counting up in seconds
        # Start counting up in seconds and update the label every second.
        self.seconds = 0  # Initialize seconds counter
        def update_label():
            # Increment seconds
            print("based")
            self.seconds += 1
            
            # Update the label text
            self.last_update_label.configure(text=f"Last Update: {self.seconds} seconds ago")
            
            # Call update_label again after 1000 ms (1 second)
            self.after(1000, update_label)
        if self.started == None:
            self.started = started
            update_label()
    

    def start_reading_events(self):
        # start_reading_events() Method:
        # Creates a randomised timer to send a reading message to the server (time period between 15 seconds and 60 seconds).

        while True:
            self.trigger_reading_event()  # Trigger the reading event.
            self.updatetime(True)
            next_bill = random.uniform(15, 60)
            print("next bill in " + str(next_bill))
            time.sleep(next_bill) # Wait random time to send next reading.
            
  



def generate_meter_reading():
    # generate_meter_reading() Function:
    # Generates a random, realistic reading value (value between 0.5 kWh and 5.0 kWh).
    return round(random.uniform(0.5, 5.0), 2)  


def send_reading_to_server(sock, reading_data):
    # send_reading_to_server() Function.
    # Function to send data to the Rust server, packs into valid JSON format & sends to server. 
    # Awaits response from server, if response received return response.

    try:
        # Convert the reading data to JSON.
        json_data = json.dumps(reading_data).encode('utf-8')

        # Calculate the length of the JSON data.
        data_length = len(json_data)

        # Pack the length as a 2-byte header (little-endian format).
        header = struct.pack('>H', data_length)

        # Send the header first, followed by the JSON data.
        sock.sendall(header + json_data)

        # Wait for the server's response. Allowing enough buffer size for the response (1024 bytes).
        response = sock.recv(1024)
        if len(response) == 0:
            return None
        # Removes header from server message using the message length.
        header = response[:2]
        message_length = struct.unpack('>H', header)[0]
        actual_message = response[2:2+message_length].decode('utf-8')
        if actual_message: # Response received.
            return actual_message
        else: # Empty Response.
            return None
        
    except socket.error as e:  #???TO BE REMOVED??? - Could this be a receive error? Could we split this up into 'Error sending data' / 'Error receiving data'.
        # Print error message if any errors are found when sending the reading to server or receiving the response.
        print(f"Error sending/rec data: {e}")
        return None


# Function to authenticate with the server
def authenticate(sock, id):
    # authenticate() Function.
    # Attempts to authenticate the client to the server. Sends an authentication message of Id number as int + string, awaits
    # response from server, and checks whether the authentication was successful ('Authentication successful').
    
    try:
        # Set maximum allowable time for the authentication response to be received.
        sock.settimeout(10)
        # Using the ID, create a JSON encoded message to be sent to the server.
        auth_message = json.dumps({"id": id,"token": str(id)}).encode('utf-8')
        # Adds 2 byte header (big-endian format)
        header = struct.pack('!H', len(auth_message))
        # Send header + authentication message to server.
        final_message = header + auth_message
        sock.sendall(final_message)

        # Wait for the server's response. Allowing enough buffer size for the response (2048 bytes).
        response = sock.recv(2048)
        # Remove the 2-byte header to get the response message.
        header = response[:2]
        message_length = struct.unpack('>H', header)[0]
        actual_message = response[2:2+message_length].decode('utf-8')
        
        # Check to see if server is alive and has successfully authenticated the client.
        if actual_message == "Authentication successful":
            print("Authentication successful")
            return True
        else:
            print("Authentication failed")
            return False
    except socket.timeout:
        # Timeout error catch.
        print("Authentication Timed out")
        return False
    except socket.error as e:
        # Other socket error catch.
        print(f"Error during authentication: {e}")
        return False


# Main function to handle communication with the server
def run_client(frame, id):
    # run_client() Function.
    # Main client function: connects to the server, authenticates and then communicates to server with readings.

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            # Try to connect to server.
            print(id)
            sock.connect((SERVER_HOST, SERVER_PORT))
            frame.set_status("Connected")
            print(f"Connected to server {SERVER_HOST}:{SERVER_PORT}")

            # Authenticate the client with the server.
            if not authenticate(sock,id):
                frame.set_status("Authentication Failed")
                return

            # Store the socket in the frame
            frame.sock = sock  
            # Start the reading events.
            frame.start_reading_events()  

        except socket.error as e:
            # If any problems occur when connecting to server it is caught and displays an error message.
            print(f"Failed to connect to server: {e}")
            frame.set_status("Connection Failed")
            time.sleep(5)
            # Tries to connect to server again -- need to put a max limit on this really -- maybe 3 tries.
            run_client(frame)


def create_client(id):
    # create_client() Function.
    # Instantiates SmartMeterGUI() class and starts the GUI event loop.
    app = SmartMeterGUI(id)
    app.mainloop()


if __name__ == "__main__":
    # Initates the smartmeter gui for x no of Clients.

    # Set default appearance mode & colour theme.
    ctk.set_appearance_mode("dark")
    ctk.set_default_color_theme("green")

    # Desired number of clients (adjustable)
    num_clients = 1
    # List to keep track of threads
    threads = []

    # Create x no of clients.
    for id in range(num_clients):
        # Start each client in a new thread.
        client_thread = threading.Thread(target=create_client, args=(id,))
        client_thread.start()
        # 1 second delay between starting each client.
        time.sleep(1)
        # Store thread reference.
        threads.append(client_thread)  

    # Wait for all threads to finish
    for thread in threads:
        thread.join()
        