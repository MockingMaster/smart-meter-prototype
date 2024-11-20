import socket
import json
import time
import random
import threading
import customtkinter as ctk
from datetime import datetime
import ssl
import multiprocessing
import struct
import queue

# Global Vars
# Declare config settings for the server
SERVER_HOST = "localhost"
SERVER_PORT = 8000
MIN_READING_INTERVAL = 15
MAX_READING_INTERVAL = 60
#modularising the program


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
        self.geometry("540x250")
        self.configure(bg="black")
        

        # Date and time display : Shows current date/time.
        self.time_label = ctk.CTkLabel(self, text="", font=("Arial", 12), text_color="white")
        self.time_label.place(x=310, y=10)  

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
        self.last_update_label.place(x=300, y=210)

        # Exit button to close the application : Closes this client.
        self.exit_button = ctk.CTkButton(self, text="Exit", command=self.exit)
        self.exit_button.place(x=5, y=210)

        # Start updating time
        self.update_time()
        

        # Start the client automatically
        threading.Thread(target=self.auto_connect, daemon=True).start()

        # Declare attributes for readings & billing info from server & assign default values.
        self.cumulative_reading = 0.0
        self.total_bill = 0.0
        self.units_start = 0.0
        self.units_end = 0.0
        self.standing_charge = 0.0
        self.price_per_unit = 0.0
        self.billing_period = 0

        # Flag to remember whether 'last updated:' status loggin has started.
        self.last_update_started = None

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
        # Set comms status back to 'Connected'.
        self.status_label.configure("Connected")


    def set_status(self, message):
        # set_status() Method:
        # Updates the status of the connection between the client and the server.
        self.status_label.configure(text=message)

    def trigger_reading_event(self):
        # trigger_reading_event() Method:
        # Generate a new reading
        new_reading = generate_meter_reading()

        # Update cumulative reading
        self.cumulative_reading += new_reading

        # Create the reading data with cumulative reading
        reading_data = {
            "type": "MeterReading",
            "reading": self.cumulative_reading,  # Send cumulative reading
        }
        return send_reading_to_server(self.sock, reading_data, 10)
        
    def start_reading_events(self):
        # start_reading_events() Method:
        # Send meter readings to the server.

        while True:
            # Trigger a reading event and send it to the server
            if len(self.trigger_reading_event()):
                break

            self.update_last_updated(True)
            # Wait for a random interval before the next reading
            next_reading_interval = random.uniform(MIN_READING_INTERVAL, MAX_READING_INTERVAL)
            time.sleep(next_reading_interval)
            


    def handle_server_message(self, message_data):
        # handle_server_message() Method:
        # Handles received JSON message & unpacks data into variables for the smart meter. Calls method to update  
        # the SmartMeterGUI elements.

        try:
            match message_data.get("type"):
                case "Bill":
                    # Handle bill data
                    bill_info = message_data

                    # Unpack bill data safely using .get() to avoid missing key issues
                    standing_charge = bill_info.get('standing_charge', 0.0)
                    total = bill_info.get('total', 0.0)  # This is the new total usage cost
                    units_start = bill_info.get('units_start', 0.0)
                    units_end = bill_info.get('units_end', 0.0)
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
                    self.units_used = units_end - units_start 

                    # Update the GUI with the new data
                    self.after(1, self.update_GUI,
                        self.total_bill,
                        self.standing_charge, 
                        self.price_per_unit,
                        self.units_used, 
                        self.billing_period)

                case "PowerGridIssue":
                    # Unpack power grid issue data safely using .get() to avoid missing key issues.
                    error_info = message_data
                    print(f"Client {self.id} {error_info}")
                    error_message = error_info.get('error', 'Unknown error')
                
                    # Update on GUI and print message.
                    print(f"Client {self.id} Power Grid Issue: {error_message}")
                    self.set_status(f"Power Grid Issue: {error_message}")
                
                case "PowerGridIssueResolved":
                    # Update on GUI and print message.
                    print(f"Client {self.id} Power Grid Issue Resolved")
                    self.set_status("Connected")

                case _:
                    # Else case.
                    # If the message type is not recognized, print a warning message.
                    print(f"Client {self.id} Unknown message type received: {message_data}")

        except Exception as e:
            print(f"Client {self.id} {e}")


    def update_last_updated(self, started):
        # update_last_updated() Method:
        # Start counting up in seconds and update 'last update:' label every second.
        
        # Initialize seconds counter
        self.last_updated_seconds = 0  
        def update_label():
            # Increment seconds.
            
            self.last_updated_seconds += 1
            
            # Update the label text.
            self.last_update_label.configure(text=f"Last Update: {self.last_updated_seconds} seconds ago")
            
            # Call update_label again after 1000 ms.
            self.after(1000, update_label)
        # sets flag to true on first pass, ensures that 'update_label' code is only executed once.
        if self.last_update_started == None:
            self.last_update_started = started
            update_label() 

    def start_listener(self):
        # Start the listener thread
        
        try:
            self.sock.settimeout(90)
            while True:
                frame = receive_frame(self.sock)
                if frame == None:
                    self.set_status("Disconnected")
                    print("Server disconnected")
                    break

                print(f"Client {self.id} Server response: {frame}")
                try:
                    # Decode the message from the server
                    message_data = json.loads(frame)
                    self.handle_server_message(message_data)
                except (json.JSONDecodeError, ValueError) as e:
                    # Error decoding server response
                    print(f"Client {self.id} Error parsing server response: {e}")
                    self.set_status("Error")
                    break
                
        except (socket.error, ssl.SSLError) as e:
            print(f"Client {self.id} Listener error: {e}")
            self.set_status("Error")
        except Exception as e:
            print(f"Client {self.id} Unexpected error in listener: {e}")
        finally:
            self.sock.shutdown(socket.SHUT_RDWR)
    
  

def generate_meter_reading():
    # generate_meter_reading() Function:
    # Generates a random, realistic reading value (value between 0.5 kWh and 2.5 kWh).
    return round(random.uniform(0.5, 2.5), 2)  


def send_reading_to_server(sock, reading_data, timeout=120):
    # send_reading_to_server() Function:
    # Sends reading to server and awaits response. 
    
    try:
        
        # Ensure the socket is still open before attempting to send data
        if sock.fileno() == -1:  # Check if the socket is closed
            print("Socket is closed, attempting to reconnect...")
            return "Error: Socket is closed"
        
            # Convert the reading data to JSON
        json_data = json.dumps(reading_data).encode('utf-8')

            # Calculate the length of the JSON data
        data_length = len(json_data)

            # Pack the length as a 2-byte header (big-endian format)
        header = struct.pack('>H', data_length)

            # Send the header first, followed by the JSON data in chunks
        total_sent = 0
        data_to_send = header + json_data

            # Ensure that all data is sent using the SSL socket
        while total_sent < len(data_to_send):
            try:
                sent = sock.send(data_to_send[total_sent:])
                if sent == 0:
                    raise RuntimeError("Socket connection broken")
                total_sent += sent
            except socket.error as e:
                print(f"Error while sending data: {e}")
                return "Error sending data to server."  # Return error message if sending fails
            
        print("Meter reading sent")
        return ""

    except socket.timeout:
        print("Timeout: Server did not respond in time.")
        return "Timeout: Server did not respond in time."  # Return timeout message if server response is delayed
    except (ssl.SSLError, socket.error) as e:
        print(f"Error sending or receiving data: {e}")
        return f"Error: {e}"  # Return socket/SSL error message
    except Exception as e:
        print(f"Unexpected error: {e}")
        return f"Unexpected error: {e}"  # Return any unexpected error message

def receive_frame(sock):
    response = sock.recv(2)
    if len(response) == 0:
        return None
    message_length = struct.unpack('>H', response)[0]
    
    response = sock.recv(message_length)
    if len(response) == 0:
        return None

    return response.decode("utf-8")

# Function to authenticate with the server
def authenticate(sock, id):
    # authenticate() Function.
    # Attempts to authenticate the client to the server. Sends an authentication message of Id number as int + string, awaits
    # response from server, and checks whether the authentication was successful ('Authentication successful').
    
    try:
        # Set maximum allowable time for the authentication response to be received.
        sock.settimeout(5)
        # Using the ID, create a JSON encoded message to be sent to the server.
        auth_message = json.dumps({"id": id,"token": str(id)}).encode('utf-8')
        # Adds 2 byte header (big-endian format)
        header = struct.pack('!H', len(auth_message))
        # Send header + authentication message to server.
        final_message = header + auth_message
        sock.sendall(final_message)

        # Wait for the server's response. Allowing enough buffer size for the response (2048 bytes).
        response = receive_frame(sock)
        if response == None:
            return False

        # Check to see if server is alive and has successfully authenticated the client.
        if response == "Authentication successful":
            print(f"Client {id} Authentication successful")
            return True
        else:
            print(f"Client {id} Authentication failed")
            return False
    except socket.timeout:
        # Timeout error catch.
        print(f"Client {id} Authentication Timed out")
        return False
    except socket.error as e:
        # Other socket error catch.
        print(f"Client {id} Error during authentication: {e}")
        return False



def run_client(frame, id, max_retries=5):
    # Main client function: connects to the server using SSL, authenticates, 
    # and then communicates with the server with readings.
    
    # Create a context that is meant for client connections
    context = ssl.create_default_context()  # Default context is for client-side communication
    
    # Load the server's certificate for verification
    context.load_verify_locations(cafile="./Certificates/server.crt")
    context.check_hostname = False

    # Verify the server's certificate (still check the validity of the server's certificate)
    context.verify_mode = ssl.CERT_REQUIRED

    reconnect_delay = 0

    while True:
        if reconnect_delay:
            time.sleep(5)

    
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            try:
                # Attempt to connect to the server
                sock.settimeout(10)
                sock.connect((SERVER_HOST, SERVER_PORT))

                # Wrap the socket with SSL
                ssl_sock = context.wrap_socket(sock, server_hostname=SERVER_HOST)

                frame.set_status("Connected")
                print(f"Client {id} Connected to server {SERVER_HOST}:{SERVER_PORT} with SSL")

                # Authenticate the client with the server
                if not authenticate(ssl_sock, id):
                    frame.set_status("Authentication Failed")
                    return

                reconnect_delay = 0

                # Store the SSL socket in the frame
                frame.sock = ssl_sock
                frame.sock.settimeout(90)
                
                # Start the reading events
                receiver = threading.Thread(target=frame.start_listener, daemon=False)
                receiver.start()
                frame.start_reading_events()
                frame.sock.shutdown(socket.SHUT_RDWR)
                receiver.join()
                frame.sock.close()
                

            except ssl.SSLError as e:
                print(f"Client {id} SSL Error: {e}")
                frame.set_status("SSL Error")
                reconnect_delay = 5
                break  # Stop retrying on SSL errors

            except socket.error as e:
                reconnect_delay = 5
                print(f"Client {id} Failed to connect to server: {e}")
                frame.set_status(f"Connection Failed, retrying")

def create_client(id):
    # create_client() Function.
    # Instantiates SmartMeterGUI() class and starts the GUI event loop.
    app = SmartMeterGUI(id)
    app.mainloop()



if __name__ == "__main__":
    # Set default appearance mode and color theme
    ctk.set_appearance_mode("dark")
    ctk.set_default_color_theme("green")

    # Desired number of clients
    num_clients = 20
    processes = []

    # Create processes to make new client gui's with a thread safe method
    for id in range(num_clients):
        # https://stackoverflow.com/questions/73208502/python-multiprocessing-with-tkinter-on-windows
        client_process = multiprocessing.Process(target=create_client, args=((id),))
        client_process.start()
        time.sleep(1)  # Delay between starting each client
        processes.append(client_process)

    # Wait for all processes to complete
    for process in processes:
        process.join()